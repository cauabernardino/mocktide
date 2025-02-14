use std::{path::Path, sync::Arc};

use log::{debug, error, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Notify, Semaphore},
    time::{self, Duration},
};

use crate::{connection::ConnHandler, mapping::MappingGuard};

const MAX_CONNECTIONS: usize = 10;

#[derive(Debug)]
pub struct TcpServer {
    mapping_guard: MappingGuard,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
}

impl TcpServer {
    pub fn new(listener: TcpListener, map_config_path: &Path) -> TcpServer {
        TcpServer {
            mapping_guard: MappingGuard::new(map_config_path),
            listener,
            limit_conns: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        }
    }
    pub async fn run(&mut self, notify: Arc<Notify>) -> crate::Result<()> {
        info!("accepting connections");

        loop {
            let permit = self.limit_conns.clone().acquire_owned().await.unwrap();
            let socket = self.accept().await?;
            let loop_notify = notify.clone();

            let mut handler = ConnHandler::new(self.mapping_guard.mapping(), socket);

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
pub async fn run_tcp_server(listener: TcpListener, map_config_path: &Path, notify: Arc<Notify>) {
    let mut server = TcpServer::new(listener, map_config_path);

    if let Err(err) = server.run(notify).await {
        error!("{}", err);
    }
}
