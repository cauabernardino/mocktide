use bytes::Bytes;
use env_logger::{Builder, Env};
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::sleep;

use mocktide::server::run_tcp_server;
use tokio::sync::Notify;

static TEST_SERVER_ADDR: &str = "127.0.0.1:6120";

fn create_mapping_file() -> NamedTempFile {
    use std::io::Write;
    let mapping: &str = r#"
        messages:
            msg1: "\x01\x65\x6C\x6C\x6F"

        actions:
            - message: msg1
              action: Recv
    "#;

    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    write!(tmpfile, "{}", mapping).unwrap();

    tmpfile
}

/// Test client for writing to server
async fn write_to_server(data: &Bytes) -> bool {
    let mut stream = TcpStream::connect(TEST_SERVER_ADDR).await.unwrap();
    stream.write_all(data).await.is_ok()
}

/// Spawn the server and returns the shutdown notifier for it
async fn test_server() -> Arc<Notify> {
    let file = create_mapping_file();
    let listener = TcpListener::bind(TEST_SERVER_ADDR).await.unwrap();
    let notify = Arc::new(Notify::new());
    let notify_here = notify.clone();

    tokio::spawn(async move { run_tcp_server(listener, file.path(), notify.clone()).await });

    notify_here
}

#[tokio::test]
async fn tcp_server_receives_expected_message() {
    let _ = Builder::from_env(Env::default().default_filter_or("info")).try_init();

    let shutdown_server = test_server().await;
    let binary_data = Bytes::from("\x01\x65\x6C\x6C\x6F");

    write_to_server(&binary_data).await;
    sleep(Duration::from_millis(100)).await; // TODO: Not ideal, improve.

    shutdown_server.notify_one();
}
