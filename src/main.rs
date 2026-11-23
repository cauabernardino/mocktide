use anyhow::{Context, Result};
use clap::Parser;
use env_logger::{Builder, Env};
use log::info;
use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;

use mocktide::cli::Cli;
use mocktide::server::{run_tcp_server, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    Builder::from_env(Env::default().default_filter_or(match args.verbose {
        0 => "info",
        1 => "debug",
        2..=u8::MAX => "debug", // TODO: Tracing
    }))
    .try_init()
    .with_context(|| "logger could not be initialized: {:#?}")?;

    let address = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&address)
        .await
        .with_context(|| format!("error binding to {}", address))?;

    let shutdown = CancellationToken::new();

    let interrupt = shutdown.clone();
    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            info!("server interrupted");
            interrupt.cancel();
        }
    });

    let config = ServerConfig {
        mapping_file_path: args.mapping_file.to_string_lossy().to_string(),
        report_path: args.report.to_string_lossy().to_string(),
        shutdown,
    };

    run_tcp_server(listener, config).await?;
    info!("server stopped");

    Ok(())
}
