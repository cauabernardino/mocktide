use std::num::NonZeroUsize;
use std::time::Duration;

use bytes::Bytes;
use env_logger::{Builder, Env};
use mocktide::server::{run_tcp_server, ServerConfig};
use tempfile::{NamedTempFile, TempPath};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const HELLO: &[u8] = b"\x48\x65\x6C\x6C\x6F";

struct TestServer {
    port: u16,
    shutdown: CancellationToken,
    handle: JoinHandle<mocktide::Result<()>>,
    report: TempPath,
    _mapping: TempPath,
}

impl TestServer {
    async fn start(mapping: &str) -> TestServer {
        let _ = Builder::from_env(Env::default().default_filter_or("debug")).try_init();

        let mut mapping_file = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut mapping_file, mapping.as_bytes()).unwrap();
        let mapping = mapping_file.into_temp_path();
        let report = NamedTempFile::new().unwrap().into_temp_path();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let shutdown = CancellationToken::new();

        let config = ServerConfig {
            mapping_file_path: mapping.to_string_lossy().to_string(),
            report_path: report.to_string_lossy().to_string(),
            max_connections: NonZeroUsize::new(10).unwrap(),
            shutdown: shutdown.clone(),
        };
        let handle = tokio::spawn(run_tcp_server(listener, config));

        TestServer {
            port,
            shutdown,
            handle,
            report,
            _mapping: mapping,
        }
    }

    async fn connect(&self) -> TcpStream {
        TcpStream::connect(("127.0.0.1", self.port)).await.unwrap()
    }

    /// Waits for the server to stop on its own and returns the written report.
    async fn finished_report(self) -> String {
        timeout(Duration::from_secs(5), self.handle)
            .await
            .expect("server did not shut down")
            .unwrap()
            .expect("server returned an error");
        assert!(self.shutdown.is_cancelled());

        std::fs::read_to_string(&self.report).unwrap()
    }
}

#[tokio::test]
async fn test_tcp_server_completes_expected_actions() {
    let server = TestServer::start(
        r#"
        name: hello
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
            reply: "\x4F\x4B\x80\xFF"
        actions:
            - message: msg1
              execute: Recv
            - message: reply
              execute: Send
            - execute: Shutdown
        "#,
    )
    .await;

    let mut client = server.connect().await;
    client.write_all(HELLO).await.unwrap();
    let mut reply = [0u8; 4];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(
        Bytes::copy_from_slice(&reply),
        Bytes::from_static(b"OK\x80\xFF")
    );

    let report = server.finished_report().await;
    // Indented: the suite starts on its own line
    assert!(
        report.contains("\n  <testsuite id=\"0\" name=\"hello\""),
        "{report}"
    );
    assert!(report.contains(r#"<testcase name="msg1""#), "{report}");
    assert!(!report.contains("<failure"), "{report}");
}

#[tokio::test]
async fn test_each_connection_gets_its_own_suite_in_one_report() {
    let server = TestServer::start(
        r#"
        name: hello
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
            reply: "\x4F\x4B"
        actions:
            - message: msg1
              execute: Recv
            - message: reply
              execute: Send
        "#,
    )
    .await;

    for _ in 0..2 {
        let mut client = server.connect().await;
        client.write_all(HELLO).await.unwrap();
        let mut reply = [0u8; 2];
        client.read_exact(&mut reply).await.unwrap();
    }
    server.shutdown.cancel();

    let report = server.finished_report().await;
    assert!(
        report.contains(r#"name="hello (connection 1)""#),
        "{report}"
    );
    assert!(
        report.contains(r#"name="hello (connection 2)""#),
        "{report}"
    );
    assert_eq!(report.matches("<testcase").count(), 2, "{report}");
    assert!(!report.contains("<failure"), "{report}");
}

#[tokio::test]
async fn test_mismatch_fails_fast_and_skips_remaining_recv() {
    let server = TestServer::start(
        r#"
        name: mismatch
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
            msg2: "\x01"
        actions:
            - message: msg1
              execute: Recv
            - message: msg2
              execute: Recv
            - execute: Shutdown
        "#,
    )
    .await;

    // A single wrong byte is enough: the server must not wait for the full length.
    let mut client = server.connect().await;
    client.write_all(b"X").await.unwrap();

    let report = server.finished_report().await;
    assert!(report.contains(r#"type="message_mismatch""#), "{report}");
    assert!(report.contains("<skipped"), "{report}");
}

#[tokio::test]
async fn test_recv_times_out() {
    let server = TestServer::start(
        r#"
        name: timeout
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
        actions:
            - message: msg1
              execute: Recv
              timeout: 1
            - execute: Shutdown
        "#,
    )
    .await;

    let _client = server.connect().await;

    let report = server.finished_report().await;
    assert!(report.contains(r#"type="timeout""#), "{report}");
}

#[tokio::test]
async fn test_report_is_written_when_peer_disconnects() {
    let server = TestServer::start(
        r#"
        name: disconnect
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
        actions:
            - message: msg1
              execute: Recv
            - execute: Shutdown
        "#,
    )
    .await;

    drop(server.connect().await);

    let report = server.finished_report().await;
    assert!(report.contains(r#"type="connection_closed""#), "{report}");
}

#[tokio::test]
async fn test_external_shutdown_aborts_pending_connection() {
    let server = TestServer::start(
        r#"
        name: interrupted
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
        actions:
            - message: msg1
              execute: Recv
        "#,
    )
    .await;

    let _client = server.connect().await;
    // Give the server time to accept and start waiting on the Recv.
    tokio::time::sleep(Duration::from_millis(100)).await;
    server.shutdown.cancel();

    let report = server.finished_report().await;
    assert!(report.contains("<skipped"), "{report}");
}

#[tokio::test]
async fn test_invalid_mapping_is_returned_as_error() {
    let server = TestServer::start(
        r#"
        name: invalid
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
        actions:
            - message: missing
              execute: Recv
        "#,
    )
    .await;

    let err = timeout(Duration::from_secs(5), server.handle)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap_err();
    assert!(format!("{err:#}").contains("missing"), "{err:#}");
}

#[tokio::test]
async fn test_failed_connection_waits_before_shutdown() {
    let server = TestServer::start(
        r#"
        name: grace
        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"
            reply: "\x4F\x4B"
        actions:
            - message: msg1
              execute: Recv
            - message: reply
              execute: Send
            - execute: Shutdown
              wait_for: 1
        "#,
    )
    .await;

    // The first connection fails right away; its pending Shutdown must still
    // honour wait_for, giving the second connection time to finish.
    let mut failing = server.connect().await;
    failing.write_all(b"X").await.unwrap();

    let mut client = server.connect().await;
    client.write_all(HELLO).await.unwrap();
    let mut reply = [0u8; 2];
    client.read_exact(&mut reply).await.unwrap();

    let report = server.finished_report().await;
    assert!(report.contains(r#"type="message_mismatch""#), "{report}");
    assert!(!report.contains("<skipped"), "{report}");
    assert_eq!(report.matches("<testcase").count(), 2, "{report}");
}
