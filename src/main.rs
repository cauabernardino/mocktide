use std::path::Path;

use anyhow::{Context, Result};
use log::{error, info};
use tokio::{net::TcpListener, signal};

use mocktide::server;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // TODO: Create CLI to receive these values
    let map_config = Path::new("./examples/config.yaml");
    if !map_config.exists() {
        error!("config file does not exist")
    }

    let host = "127.0.0.1";
    let port = 6000;
    let listener = TcpListener::bind(&format!("{}:{}", host, port))
        .await
        .with_context(|| format!("error binding to {}", &port))?;

    info!("server will start in port {}", port);
    server::run_tcp_server(listener, map_config, signal::ctrl_c()).await;

    Ok(())
}
