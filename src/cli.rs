use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to the mapping file
    pub mapping_file: PathBuf,

    /// Verbosity level [default: info]
    /// options: -v: debug
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Server port
    #[arg(short, long, default_value = "6020")]
    pub port: u16,
}
