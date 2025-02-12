use anyhow::{anyhow, Context, Result};
use clap::Parser;
use env_logger::{Builder, Env};
use log::info;
use tokio::{net::TcpListener, signal};

use mocktide::cli;
use mocktide::server;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();

    Builder::from_env(Env::default().default_filter_or(match args.verbose {
        0 => "info",
        1 => "debug",
        2..=u8::MAX => "debug", // TODO: Tracing
    }))
    .try_init()
    .with_context(|| "logger could not be initialized: {:#?}")?;

    if !args.mapping_file.exists() {
        return Err(anyhow!("file {:#?} does not exist", args.mapping_file));
    }

    let address = format!("{}:{}", &args.host, &args.port);
    let listener = TcpListener::bind(&address)
        .await
        .with_context(|| format!("error binding to {}", &address))?;

    info!("server will start in address {}", &address);
    server::run_tcp_server(listener, args.mapping_file.as_path(), signal::ctrl_c()).await;

    Ok(())
}
