use std::sync::Arc;

use log::{debug, error, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, Semaphore},
    time::{self, Duration},
};

use crate::connection::Connection;
use crate::mapping::{Mapping, MappingDropGuard};
use crate::message::Message;
use crate::shutdown::Shutdown;

const MAX_CONNECTIONS: usize = 10;

#[derive(Debug)]
pub struct TcpServer {
    mapping_guard: MappingDropGuard,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
    shutdown_channel: broadcast::Sender<()>,
    // shutdown_complete_tx: mpsc::Sender<()>,
}

#[derive(Debug)]
struct Handler {
    mapping: Mapping,
    conn: Connection,
    shutdown: Shutdown,
    // _shutdown_recv: mpsc::Sender<()>,
}

impl TcpServer {
    pub fn new(listener: TcpListener) -> TcpServer {
        TcpServer {
            mapping_guard: MappingDropGuard::new(),
            listener,
            limit_conns: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            shutdown_channel: broadcast::channel(1).0,
            // shutdown_complete_tx: ,
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
                shutdown: Shutdown::new(self.shutdown_channel.subscribe()),
                // _shutdown_recv: self.shutdown_complete_tx.clone(),
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!("connection error: {:}", err);
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

impl Handler {
    async fn run(&mut self) -> crate::Result<()> {
        while !self.shutdown.is_shutdown() {
            let maybe_msg = tokio::select! {
                res = self.conn.read_message() => res?,
                _ = self.shutdown.recv() => {
                    return Ok(());
                }
            };

            let msg = match maybe_msg {
                Some(msg) => msg,
                None => return Ok(()),
            };

            reply(&self.mapping, &mut self.conn, &msg).await?;
        }

        Ok(())
    }
}

pub(crate) async fn reply(
    _mapping: &Mapping,
    dst: &mut Connection,
    msg: &Message,
) -> crate::Result<()> {
    // mapping.set("test".to_string(), msg);
    debug!("{:}", msg);
    dst.write_message(msg).await?;

    Ok(())
}
