use std::sync::{Arc, Mutex};

use junit_report::TestSuite;
use log::{debug, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::{self, Duration},
};
use tokio_util::task::TaskTracker;

use crate::{connection::ConnHandler, mapping::Mapping, reporter};

use super::ServerConfig;

#[derive(Debug)]
pub struct TcpServer {
    mapping: Arc<Mapping>,
    listener: TcpListener,
    limit_conns: Arc<Semaphore>,
    connections: TaskTracker,
    /// Finished connection suites, keyed by accept order
    suites: Arc<Mutex<Vec<(usize, TestSuite)>>>,
    config: ServerConfig,
}

impl TcpServer {
    /// Fails if the mapping file cannot be read or is invalid.
    pub fn new(listener: TcpListener, config: ServerConfig) -> crate::Result<TcpServer> {
        Ok(TcpServer {
            mapping: Arc::new(Mapping::from_file(&config.mapping_file_path)?),
            listener,
            limit_conns: Arc::new(Semaphore::new(config.max_connections.get())),
            connections: TaskTracker::new(),
            suites: Arc::default(),
            config,
        })
    }

    /// Accepts connections until shutdown, waits for in-flight connections to
    /// finish, then writes the report with every connection's results.
    pub async fn run(&self) -> crate::Result<()> {
        info!("accepting connections on {}", self.listener.local_addr()?);

        let result = tokio::select! {
            _ = self.config.shutdown.cancelled() => Ok(()),
            result = self.accept_loop() => result,
        };

        self.connections.close();
        self.connections.wait().await;

        let report = self.write_report();
        result.and(report)
    }

    async fn accept_loop(&self) -> crate::Result<()> {
        let mut next_index = 0;
        loop {
            let permit = self
                .limit_conns
                .clone()
                .acquire_owned()
                .await
                .expect("semaphore is never closed");
            let socket = self.accept().await?;

            let index = next_index;
            next_index += 1;

            let mut handler = ConnHandler::new(self.mapping.clone(), socket);
            let shutdown = self.config.shutdown.clone();
            let suites = self.suites.clone();

            self.connections.spawn(async move {
                let suite = handler.run(shutdown).await;
                suites.lock().unwrap().push((index, suite));
                drop(permit);
            });
        }
    }

    async fn accept(&self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            match self.listener.accept().await {
                Ok((socket, peer)) => {
                    debug!("connected: {peer}");
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

    fn write_report(&self) -> crate::Result<()> {
        let mut finished = std::mem::take(&mut *self.suites.lock().unwrap());
        finished.sort_by_key(|(index, _)| *index);
        let suites = finished.into_iter().map(|(_, suite)| suite).collect();

        info!("writing report to '{}'", self.config.report_path);
        reporter::write_report(&self.config.report_path, suites)
    }
}

/// Entry point for running the TCP server. Returns once shutdown is requested,
/// all connections have finished and the report is written.
pub async fn run_tcp_server(listener: TcpListener, config: ServerConfig) -> crate::Result<()> {
    TcpServer::new(listener, config)?.run().await
}
