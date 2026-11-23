use bytes::{Buf, Bytes, BytesMut};
use junit_report::TestSuite;
use log::{debug, error, info};
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio::time::{self, Instant};
use tokio_util::sync::CancellationToken;

use crate::mapping::{Action, Mapping, MessageAction};
use crate::reporter::Reporter;

/// Connection holds the interaction between server and peer
#[derive(Debug)]
pub(crate) struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

/// ConnHandler handles a single connection logic
#[derive(Debug)]
pub(crate) struct ConnHandler {
    mapping: Arc<Mapping>,
    conn: Connection,
}

#[derive(Debug)]
pub enum MessageError {
    /// Peer closed the connection before the expected message arrived
    Closed,

    /// Messages do not match
    NotEqual,

    /// Expected message did not arrive in time
    Timeout,

    /// Error reading from or writing to the socket
    Io(io::Error),
}

impl Connection {
    pub fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(8 * 1024),
        }
    }

    /// Reads from the stream until `expected` is received, failing as soon as
    /// the received bytes diverge from it.
    pub async fn recv(&mut self, expected: &[u8]) -> Result<(), MessageError> {
        loop {
            let received = &self.buffer[..self.buffer.len().min(expected.len())];
            debug!("expected: {:?}\tbuffer: {:?}", expected, received);

            if !expected.starts_with(received) {
                return Err(MessageError::NotEqual);
            }
            if received.len() == expected.len() {
                self.buffer.advance(expected.len());
                return Ok(());
            }

            if self.stream.read_buf(&mut self.buffer).await? == 0 {
                return Err(MessageError::Closed);
            }
        }
    }

    /// Sends message to the stream.
    pub async fn send(&mut self, msg_name: &str, msg: &Bytes) -> Result<(), MessageError> {
        self.stream.write_all(msg).await?;
        self.stream.flush().await?;
        info!("send '{:}': {:#?}", msg_name, msg);

        Ok(())
    }
}

impl ConnHandler {
    pub fn new(mapping: Arc<Mapping>, socket: TcpStream) -> ConnHandler {
        ConnHandler {
            mapping,
            conn: Connection::new(socket),
        }
    }

    /// Executes the mapped actions in order, stopping at the first failure or when
    /// `shutdown` is cancelled. Remaining `Recv` actions are reported as skipped and
    /// a remaining `Shutdown` still takes effect after its `wait_for`, so a failed run
    /// never leaves the server hanging nor cuts other connections short.
    /// Returns this connection's results.
    pub async fn run(&mut self, shutdown: CancellationToken) -> TestSuite {
        let conn = &mut self.conn;
        let mapping = &self.mapping;

        let mut reporter = Reporter::new(&mapping.name);
        let mut actions = mapping.actions.iter();
        // Delay before signalling shutdown, once a `Shutdown` action is reached
        let mut shutdown_after: Option<u64> = None;

        for MessageAction { execute, wait_for } in actions.by_ref() {
            let step = async {
                if *wait_for != 0 {
                    info!("waiting for {} seconds", wait_for);
                    time::sleep(Duration::from_secs(*wait_for)).await;
                }
                let start = Instant::now();
                let result = perform(conn, mapping, execute).await;
                (result, start.elapsed())
            };

            let (result, elapsed) = tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("server shutting down, aborting remaining actions");
                    if let Action::Recv { message, .. } = execute {
                        reporter.skipped(message);
                    }
                    break;
                }
                outcome = step => outcome,
            };

            match (execute, result) {
                (Action::Shutdown, _) => {
                    // Its wait_for already elapsed as part of the step
                    shutdown_after = Some(0);
                    break;
                }
                (Action::Send { .. }, Ok(())) => {}
                (Action::Recv { message, .. }, Ok(())) => {
                    info!("message '{:}' was recv correctly", message);
                    reporter.success(message, elapsed);
                }
                (Action::Send { message }, Err(err)) => {
                    error!("failed to send '{:}': {:}", message, err);
                    reporter.error(message, elapsed, "send_error", &err.to_string());
                    break;
                }
                (Action::Recv { message, .. }, Err(MessageError::Io(err))) => {
                    error!("failed to recv '{:}': {:}", message, err);
                    reporter.error(message, elapsed, "recv_error", &err.to_string());
                    break;
                }
                (Action::Recv { message, .. }, Err(err)) => {
                    error!("message '{:}' was not recv correctly: {:}", message, err);
                    reporter.failure(message, elapsed, err.kind(), &err.to_string());
                    break;
                }
            }
        }

        for MessageAction { execute, wait_for } in actions {
            match execute {
                Action::Recv { message, .. } => reporter.skipped(message),
                Action::Shutdown => {
                    shutdown_after.get_or_insert(*wait_for);
                }
                Action::Send { .. } => {}
            }
        }

        if let Some(wait_for) = shutdown_after {
            if wait_for != 0 {
                info!("shutting down in {} seconds", wait_for);
            }
            tokio::select! {
                _ = shutdown.cancelled() => {}
                _ = time::sleep(Duration::from_secs(wait_for)) => {
                    info!("notifying shutdown");
                    shutdown.cancel();
                }
            }
        }

        reporter.into_suite()
    }
}

async fn perform(
    conn: &mut Connection,
    mapping: &Mapping,
    action: &Action,
) -> Result<(), MessageError> {
    match action {
        Action::Send { message } => conn.send(message, &mapping.messages[message]).await,
        Action::Recv { message, timeout } => {
            let expected = &mapping.messages[message];
            time::timeout(Duration::from_secs(*timeout), conn.recv(expected))
                .await
                .map_err(|_| MessageError::Timeout)?
        }
        Action::Shutdown => Ok(()),
    }
}

impl MessageError {
    /// Short identifier used as the failure type in reports
    fn kind(&self) -> &'static str {
        match self {
            MessageError::Closed => "connection_closed",
            MessageError::NotEqual => "message_mismatch",
            MessageError::Timeout => "timeout",
            MessageError::Io(_) => "io_error",
        }
    }
}

impl From<io::Error> for MessageError {
    fn from(err: io::Error) -> Self {
        MessageError::Io(err)
    }
}

impl fmt::Display for MessageError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MessageError::Closed => "connection closed by peer".fmt(fmt),
            MessageError::NotEqual => "messages do not match".fmt(fmt),
            MessageError::Timeout => "timed out waiting for message".fmt(fmt),
            MessageError::Io(err) => err.fmt(fmt),
        }
    }
}
