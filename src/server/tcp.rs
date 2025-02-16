use std::sync::Arc;

use log::{debug, error, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::{self, Duration},
};

use crate::{connection::ConnHandler, mapping::MappingGuard};

use super::ServerConfig;

const MAX_CONNECTIONS: usize = 10;

#[derive(Debug)]
pub struct TcpServer {
    mapping_guard: MappingGuard,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
    config: ServerConfig,
}

impl TcpServer {
    pub fn new(listener: TcpListener, config: ServerConfig) -> TcpServer {
        TcpServer {
            mapping_guard: MappingGuard::new(config.mapping_file_path.clone()),
            listener,
            limit_conns: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            config,
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        info!("accepting connections");

        loop {
            let permit = self.limit_conns.clone().acquire_owned().await.unwrap();
            let socket = self.accept().await?;
            let loop_notify = self.config.shutdown_notify.clone();

            let mut handler = ConnHandler::new(
                self.mapping_guard.mapping(),
                socket,
                self.config.report_path.clone(),
            );

            tokio::spawn(async move {
                if let Err(err) = handler.run(loop_notify).await {
                    error!("error: {:}", err);
                }

                drop(permit);
            });
        }
    }

    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            match self.listener.accept().await {
                Ok((socket, _)) => {
                    debug!("connected: {:?}", &socket.peer_addr()?);
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

/// Entry point for running the TCP server.
pub async fn run_tcp_server(listener: TcpListener, config: ServerConfig) {
    let mut server = TcpServer::new(listener, config);

    if let Err(err) = server.run().await {
        error!("{}", err);
    }
}
