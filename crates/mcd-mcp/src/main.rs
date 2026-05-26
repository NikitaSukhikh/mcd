//! Command line entry point for the MCD MCP server.

use anyhow::Result;
use clap::{Parser, ValueEnum};

/// Run the MCD Model Context Protocol server.
#[derive(Debug, Parser)]
#[command(name = "mcd-mcp", version, about = "MCD Model Context Protocol server")]
struct Cli {
    /// MCP transport to use.
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    transport: Transport,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Transport {
    /// Newline-delimited JSON-RPC over stdin/stdout.
    Stdio,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.transport {
        Transport::Stdio => mcd_mcp::run_stdio(),
    }
}
