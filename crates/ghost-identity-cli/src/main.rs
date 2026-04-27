//! Smoke-test CLI for ghost-identity. NOT shipped in the eventual desktop app —
//! purely a manual verification tool during MVP-1 development.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ghost-identity")]
#[command(about = "Ghost identity smoke-test CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print version info and exit.
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("ghost-identity-cli {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
