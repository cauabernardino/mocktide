pub mod cli;
pub mod connection;
pub mod mapping;
pub mod reporter;
pub mod server;

pub type Result<T> = anyhow::Result<T, anyhow::Error>;
