use anyhow::{Context, Result};
use log::{error, info};
use mocktide::server::TcpServer;
use tokio::{net::TcpListener, signal};

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: improve logger init
    env_logger::init();

    let port = 6000;

    let listener = TcpListener::bind(&format!("127.0.0.1:{}", port))
        .await
        .with_context(|| format!("error binding to {}", &port))?;

    let mut server = TcpServer::new(listener);

    info!("Server will start in port {}", port);

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                error!("{}", err);
            }
        }
        _ = signal::ctrl_c() => {
            info!("shutting down!");
        }
    }

    Ok(())
}
