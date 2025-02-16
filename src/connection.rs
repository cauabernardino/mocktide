use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use log::{debug, error, info};
use std::fmt;
use std::io::{self, Cursor};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::time::{sleep, Instant};

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
    mapping: Mapping,
    conn: Connection,
    report_path: String,
}

#[derive(Debug)]
pub enum MessageError {
    /// Not enough data is available to parse a message
    Incomplete,

    /// Messages do not match
    NotEqual,

    /// Error in buffer
    BufferError,

    /// Invalid message encoding
    Other(anyhow::Error),
}

impl Connection {
    pub fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(8 * 1024),
        }
    }

    /// Receives a message from the stream and checks if match with the one expected.
    pub async fn recv(&mut self, expected_message: &Bytes) -> Result<Option<usize>, MessageError> {
        loop {
            if let Some(len) = self.check_recv(expected_message)? {
                return Ok(Some(len));
            }

            if 0 == self
                .stream
                .read_buf(&mut self.buffer)
                .await
                .map_err(|_| MessageError::BufferError)?
            {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(MessageError::Other(anyhow!("connection reset by peer")));
                }
            }
        }
    }

    fn check_recv(&mut self, expected_message: &Bytes) -> Result<Option<usize>, MessageError> {
        let buf_cursor = Cursor::new(&self.buffer[..]);

        let expected_len = expected_message.len();
        let buf_len = buf_cursor.get_ref().len();

        let recv = Bytes::copy_from_slice(&buf_cursor.get_ref()[..buf_len]);

        if buf_len < expected_len {
            return Ok(None);
        }
        debug!("expected: {:?}\tbuffer: {:?}", expected_message, recv);

        if *expected_message == recv.slice(..expected_len) {
            self.buffer = self.buffer.split_off(expected_len);
            return Ok(Some(expected_len));
        }

        Err(MessageError::NotEqual)
    }

    /// Sends message to the stream.
    pub async fn send(&mut self, msg_name: &String, msg: &Bytes) -> Result<(), MessageError> {
        self.write_message(msg)
            .await
            .map_err(|_| MessageError::BufferError)?;
        info!("send '{:}': {:#?}", msg_name, msg);

        Ok(())
    }

    pub async fn write_message(&mut self, message: &Bytes) -> io::Result<()> {
        self.stream.write_all(message).await?;
        self.stream.flush().await
    }
}

impl ConnHandler {
    pub fn new(mapping: Mapping, socket: TcpStream, report_path: String) -> ConnHandler {
        ConnHandler {
            mapping,
            conn: Connection::new(socket),
            report_path,
        }
    }

    pub async fn run(&mut self, notify: Arc<Notify>) -> Result<(), MessageError> {
        let mapping = self.mapping.state.try_read().unwrap();

        let mut reporter = Reporter::new(&mapping.mapping_name);

        for next_action in &mapping.message_actions {
            let MessageAction {
                message,
                action,
                wait_for,
            } = next_action;
            let msg_value = &mapping.name_to_message[message];

            let start_action = Instant::now();

            if *wait_for != 0 {
                info!("waiting for {} seconds", *wait_for);
                sleep(Duration::from_secs(*wait_for)).await;
            }

            match action {
                Action::Shutdown => {
                    info!("notifying shutdown");
                    notify.notify_waiters()
                }
                Action::Send => self.conn.send(message, msg_value).await?,
                Action::Recv => {
                    match self.conn.recv(msg_value).await {
                        Ok(Some(_)) => {
                            info!("message '{:}' was recv correctly", message);
                            reporter.sucess(message, start_action.elapsed());
                        }
                        Ok(None) => {
                            error!("message '{:}' was not recv correctly", message);
                            reporter.failure(
                                message,
                                start_action.elapsed(),
                                "recv_error",
                                "message not recv correctly",
                            );
                        }
                        Err(e) => {
                            error!("{:}", e);
                            reporter.error(
                                message,
                                start_action.elapsed(),
                                "message_error",
                                format!("{:}", e).as_str(),
                            );
                        }
                    };
                }
                Action::Unknown => unimplemented!(),
            };
        }
        reporter.report(self.report_path.as_str());

        Ok(())
    }
}

impl fmt::Display for MessageError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MessageError::Incomplete => "incomplete message in stream".fmt(fmt),
            MessageError::NotEqual => "messages do not match".fmt(fmt),
            MessageError::BufferError => "error in reading or writing to buffer".fmt(fmt),
            MessageError::Other(err) => err.fmt(fmt),
        }
    }
}
