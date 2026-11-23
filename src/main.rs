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
    .context("logger could not be initialized")?;

    let address = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&address)
        .await
        .with_context(|| format!("error binding to {}", address))?;

    let shutdown = CancellationToken::new();

    let interrupt = shutdown.clone();
    tokio::spawn(async move {
        interrupted().await;
        info!("server interrupted");
        interrupt.cancel();
    });

    let config = ServerConfig {
        mapping_file_path: args.mapping_file.to_string_lossy().to_string(),
        report_path: args.report.to_string_lossy().to_string(),
        max_connections: args.max_connections,
        shutdown,
    };

    run_tcp_server(listener, config).await?;
    info!("server stopped");

    Ok(())
}

/// Resolves on Ctrl-C, or on SIGTERM on Unix, which is what `kill`, `timeout`,
/// Docker and CI runners send.
async fn interrupted() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut terminate =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            Ok(()) = signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        if signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
