use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::fmt;
use std::io::{self, Cursor};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
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
        debug!("expected: {:?}\trecv: {:?}", expected_message, recv);

        if *expected_message == recv.slice(..expected_len) {
            self.buffer = self.buffer.split_off(expected_len);
            return Ok(Some(expected_len));
        }

        Err(MessageError::NotEqual)
    }
    /// Sends message to the stream.
    pub async fn send(&mut self, msg: &Bytes) -> Result<(), MessageError> {
        self.write_message(msg)
            .await
            .map_err(|_| MessageError::BufferError)?;
        debug!("send: {:?}", msg);

        Ok(())
    }

    pub async fn write_message(&mut self, message: &Bytes) -> io::Result<()> {
        self.stream.write_all(message).await?;
        self.stream.flush().await
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
