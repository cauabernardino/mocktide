use anyhow::Result;
use bytes::{Buf, Bytes, BytesMut};
use log::{error, info};
use std::io::Cursor;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
};

#[derive(Debug)]
pub struct Processor {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("incomplete data")]
    Incomplete,

    #[error("connection reset by peer")]
    ConnectionReset,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Processor {
    pub fn new(socket: TcpStream) -> Processor {
        Processor {
            stream: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(8 * 1024),
        }
    }

    /// Process the incoming data
    pub async fn process_incoming(&mut self) -> Result<Option<Bytes>, ProcessError> {
        loop {
            if let Some(msg) = self.parse_incoming()? {
                return Ok(Some(msg));
            }

            if 0 == self
                .stream
                .read_buf(&mut self.buffer)
                .await
                .map_err(|e| ProcessError::Other(e.into()))?
            {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(ProcessError::ConnectionReset);
                }
            }
        }
    }

    fn parse_incoming(&mut self) -> Result<Option<Bytes>, ProcessError> {
        // This will be actually used later with specific protocol parsing
        let mut buf_cursor = Cursor::new(&self.buffer[..]);

        match get_message(&mut buf_cursor) {
            Ok(msg) => {
                let len = buf_cursor.position() as usize;
                buf_cursor.set_position(0);

                self.buffer.advance(len);
                Ok(Some(msg))
            }
            Err(ProcessError::Incomplete) => Ok(None),
            Err(e) => Err(ProcessError::Other(e.into())),
        }
    }

    /// Writes to the stream
    pub async fn write(&mut self, msg: Bytes) {
        if let Err(e) = self.stream.write_all(msg.iter().as_slice()).await {
            error!("error writing to the stream: {:}", e)
        }
    }

    pub async fn ping(&mut self) {
        let buf = Bytes::copy_from_slice(b"ping");
        self.write(buf).await;
    }
}

/// Parses a message in the data stream
fn get_message(cursor: &mut Cursor<&[u8]>) -> Result<Bytes, ProcessError> {
    let msg = &cursor.get_ref()[..];
    if !msg.len() == 0 {
        return Ok(Bytes::copy_from_slice(msg));
    }

    Err(ProcessError::Incomplete)
}
