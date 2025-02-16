mod tcp;

use std::sync::Arc;
use tokio::sync::Notify;

pub use std::path::Path;
pub use tcp::run_tcp_server;
pub use tcp::TcpServer;

/// Wrapper for server configuration
#[derive(Debug)]
pub struct ServerConfig {
    pub mapping_file_path: String,
    pub report_path: String,
    pub shutdown_notify: Arc<Notify>,
}
