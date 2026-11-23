mod tcp;

use std::num::NonZeroUsize;

use tokio_util::sync::CancellationToken;

pub use tcp::run_tcp_server;
pub use tcp::TcpServer;

/// Wrapper for server configuration
#[derive(Debug)]
pub struct ServerConfig {
    pub mapping_file_path: String,
    pub report_path: String,
    /// Maximum number of connections served at the same time
    pub max_connections: NonZeroUsize,
    /// Cancelled to stop the server, either externally or by a `Shutdown` action
    pub shutdown: CancellationToken,
}
