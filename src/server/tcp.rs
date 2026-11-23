use std::sync::Arc;

use log::{debug, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::{self, Duration},
};
use tokio_util::task::TaskTracker;

use crate::{connection::ConnHandler, mapping::MappingGuard};

use super::ServerConfig;

const MAX_CONNECTIONS: usize = 10;

#[derive(Debug)]
pub struct TcpServer {
    mapping_guard: MappingGuard,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
    connections: TaskTracker,
    config: ServerConfig,
}

impl TcpServer {
    /// Fails if the mapping file cannot be read or is invalid.
    pub fn new(listener: TcpListener, config: ServerConfig) -> crate::Result<TcpServer> {
        Ok(TcpServer {
            mapping_guard: MappingGuard::new(&config.mapping_file_path)?,
            listener,
            limit_conns: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            connections: TaskTracker::new(),
            config,
        })
    }

    /// Accepts connections until shutdown, then waits for in-flight connections
    /// to finish writing their reports.
    pub async fn run(&self) -> crate::Result<()> {
        info!("accepting connections on {}", self.listener.local_addr()?);

        let result = tokio::select! {
            _ = self.config.shutdown.cancelled() => Ok(()),
            result = self.accept_loop() => result,
        };

        self.connections.close();
        self.connections.wait().await;

        result
    }

    async fn accept_loop(&self) -> crate::Result<()> {
        loop {
            let permit = self
                .limit_conns
                .clone()
                .acquire_owned()
                .await
                .expect("semaphore is never closed");
            let socket = self.accept().await?;

            let mut handler = ConnHandler::new(
                self.mapping_guard.mapping(),
                socket,
                self.config.report_path.clone(),
            );
            let shutdown = self.config.shutdown.clone();

            self.connections.spawn(async move {
                handler.run(shutdown).await;
                drop(permit);
            });
        }
    }

    async fn accept(&self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            match self.listener.accept().await {
                Ok((socket, _)) => {
                    debug!("connected: {:?}", socket.peer_addr()?);
                    return Ok(socket);
                }
                Err(err) => {
                    if backoff > 64 {
                        return Err(err.into());
                    }
                }
            }

            time::sleep(Duration::from_secs(backoff)).await;
            backoff *= 2;
        }
    }
}

/// Entry point for running the TCP server. Returns once shutdown is requested
/// and all connections have finished.
pub async fn run_tcp_server(listener: TcpListener, config: ServerConfig) -> crate::Result<()> {
    TcpServer::new(listener, config)?.run().await
}
