pub mod connection;
pub mod message;
pub mod processor;
pub mod server;

mod mapping;
mod shutdown;

pub const DEFAULT_PORT: u16 = 6000;

// TODO: Update errors to use Anyhow
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
