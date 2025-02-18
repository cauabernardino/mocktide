use std::sync::Arc;

use bytes::Bytes;
use claims::assert_ok;
use env_logger::{Builder, Env};
use log::info;
use mocktide::server::{run_tcp_server, ServerConfig};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

fn create_mapping_file() -> NamedTempFile {
    use std::io::Write;
    let mapping: &str = r#"
        name: hello

        messages:
            msg1: "\x48\x65\x6C\x6C\x6F"

        actions:
            - message: msg1
              execute: Recv
            - execute: Shutdown
    "#;

    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    write!(tmpfile, "{}", mapping).unwrap();

    tmpfile
}

struct TestServer {
    port: u16,
    shutdown: Arc<Notify>,
}

/// Spawn the server and returns the shutdown notifier for it
async fn test_server() -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let shutdown_notify = Arc::new(Notify::new());

    let shutdown = shutdown_notify.clone();

    tokio::spawn(async move {
        let mapping_file = create_mapping_file();
        let report_file = tempfile::NamedTempFile::new().unwrap();

        let mapping_file_path = mapping_file.path().to_string_lossy().to_string();
        let report_path = report_file.path().to_string_lossy().to_string();

        let config = ServerConfig {
            mapping_file_path,
            report_path,
            shutdown_notify,
        };
        run_tcp_server(listener, config).await
    });

    TestServer { port, shutdown }
}

/// Test client for writing to server
async fn write_to_server(server_port: u16, data: &Bytes) -> Result<(), std::io::Error> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", server_port))
        .await
        .unwrap();
    stream.write_all(data).await
}

/// Right now, just testing if a client can write to server.
/// Will be improve when output of results is made.
#[tokio::test]
async fn test_tcp_server_completes_expected_actions() {
    let test_server = test_server().await;
    let _ = Builder::from_env(Env::default().default_filter_or("debug")).try_init();
    info!("test server port: {}", &test_server.port);

    let res = write_to_server(test_server.port, &Bytes::from("\x48\x65\x6C\x6C\x6F")).await;

    test_server.shutdown.notified().await;
    assert_ok!(res);
}
