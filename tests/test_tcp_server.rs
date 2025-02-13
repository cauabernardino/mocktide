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

async fn test_client(notify: Arc<Notify>) {
    let server_address = "127.0.0.1:6120";
    let mut stream = TcpStream::connect(server_address).await.unwrap();
    let binary_data = Bytes::from("\x01\x65\x6C\x6C\x6F");
    let _ = stream.write_all(&binary_data).await;

    sleep(Duration::from_millis(100)).await; // TODO: Not ideal, improve.
    notify.notify_one();
}

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

#[tokio::test]
async fn tcp_server_receives_expected_message() {
    let _ = Builder::from_env(Env::default().default_filter_or("debug")).try_init();

    let file = create_mapping_file();
    let address = "127.0.0.1:6120";
    let listener = TcpListener::bind(&address).await.unwrap();
    let notify = Arc::new(Notify::new());

    let notify_here = notify.clone();

    let server_task =
        tokio::spawn(async move { run_tcp_server(listener, file.path(), notify.clone()).await });

    test_client(notify_here.clone()).await;

    let _ = tokio::join!(server_task);
}
