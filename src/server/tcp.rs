use std::{future::Future, path::Path, sync::Arc};

use log::{error, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, Semaphore},
    time::{self, Duration},
};

use crate::{
    connection::{Connection, MessageError},
    mapping::{Action, Mapping, MappingGuard, MessageAction},
};

const MAX_CONNECTIONS: usize = 10;

#[derive(Debug)]
pub struct TcpServer {
    mapping_guard: MappingGuard,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
    shutdown_channel: broadcast::Sender<()>,
}

#[derive(Debug)]
struct Handler {
    mapping: Mapping,
    conn: Connection,
}

impl TcpServer {
    pub fn new(listener: TcpListener, map_config_path: &Path) -> TcpServer {
        TcpServer {
            mapping_guard: MappingGuard::new(map_config_path),
            listener,
            limit_conns: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            shutdown_channel: broadcast::channel(1).0,
        }
    }
    pub async fn run(&mut self) -> crate::Result<()> {
        info!("accepting connections");

        loop {
            let permit = self.limit_conns.clone().acquire_owned().await.unwrap();
            let socket = self.accept().await?;

            let mut handler = Handler {
                mapping: self.mapping_guard.mapping(),
                conn: Connection::new(socket),
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
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
                Ok((socket, _)) => return Ok(socket),
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

/// Handler handles the connection logic
impl Handler {
    async fn run(&mut self) -> Result<(), MessageError> {
        for next_action in &self.mapping.state.message_actions {
            let MessageAction {
                message, action, ..
            } = next_action;

            let msg_value = &self.mapping.state.name_to_message[message];

            match action {
                Action::Send => self.conn.send(msg_value).await?,
                Action::Recv => {
                    let maybe_recv = self.conn.read_message(msg_value).await?;

                    match maybe_recv {
                        Some(_) => info!("message '{:}' was recv correctly", message),
                        None => error!("message '{:}' was not recv correctly", message),
                    };
                }
                Action::Unknown => unimplemented!(),
            };
        }

        Ok(())
    }
}

/// Entry point for running the TCP server.
pub async fn run_tcp_server(listener: TcpListener, map_config_path: &Path, shutdown: impl Future) {
    let mut server = TcpServer::new(listener, map_config_path);

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                error!("{}", err);
            }
        }
        _ = shutdown => {
            info!("shutting down!");
        }
    }

    let TcpServer {
        shutdown_channel, ..
    } = server;

    drop(shutdown_channel);
}
