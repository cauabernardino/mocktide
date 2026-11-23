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
    /// a remaining `Shutdown` still takes effect, so a failed run never leaves the
    /// server hanging. Returns this connection's results.
    pub async fn run(&mut self, shutdown: CancellationToken) -> TestSuite {
        let conn = &mut self.conn;
        let mapping = &self.mapping;

        let mut reporter = Reporter::new(&mapping.name);
        let mut actions = mapping.actions.iter();
        let mut shutdown_requested = false;

        for action in actions.by_ref() {
            let MessageAction {
                message, execute, ..
            } = action;

            let step = async {
                if action.wait_for != 0 {
                    info!("waiting for {} seconds", action.wait_for);
                    time::sleep(Duration::from_secs(action.wait_for)).await;
                }
                let start = Instant::now();
                let result = perform(conn, action, &mapping.messages[message]).await;
                (result, start.elapsed())
            };

            let (result, elapsed) = tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("server shutting down, aborting remaining actions");
                    if *execute == Action::Recv {
                        reporter.skipped(message);
                    }
                    break;
                }
                outcome = step => outcome,
            };

            match (execute, result) {
                (Action::Shutdown, _) => {
                    shutdown_requested = true;
                    break;
                }
                (Action::Send, Ok(())) => {}
                (Action::Recv, Ok(())) => {
                    info!("message '{:}' was recv correctly", message);
                    reporter.success(message, elapsed);
                }
                (Action::Send, Err(err)) => {
                    error!("failed to send '{:}': {:}", message, err);
                    reporter.error(message, elapsed, "send_error", &err.to_string());
                    break;
                }
                (Action::Recv, Err(MessageError::Io(err))) => {
                    error!("failed to recv '{:}': {:}", message, err);
                    reporter.error(message, elapsed, "recv_error", &err.to_string());
                    break;
                }
                (Action::Recv, Err(err)) => {
                    error!("message '{:}' was not recv correctly: {:}", message, err);
                    reporter.failure(message, elapsed, err.kind(), &err.to_string());
                    break;
                }
            }
        }

        for action in actions {
            match action.execute {
                Action::Recv => reporter.skipped(&action.message),
                Action::Shutdown => shutdown_requested = true,
                Action::Send => {}
            }
        }

        if shutdown_requested {
            info!("notifying shutdown");
            shutdown.cancel();
        }

        reporter.into_suite()
    }
}

async fn perform(
    conn: &mut Connection,
    action: &MessageAction,
    msg: &Bytes,
) -> Result<(), MessageError> {
    match action.execute {
        Action::Send => conn.send(&action.message, msg).await,
        Action::Recv => time::timeout(Duration::from_secs(action.timeout), conn.recv(msg))
            .await
            .map_err(|_| MessageError::Timeout)?,
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
