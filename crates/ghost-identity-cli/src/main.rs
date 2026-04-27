//! Smoke-test CLI for ghost-identity. NOT shipped in the eventual desktop app —
//! purely a manual verification tool during MVP-1 development.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ghost_core::Fingerprint;
use ghost_identity::{identity_file, CreateOptions, Identity};

#[derive(Parser)]
#[command(name = "ghost-identity")]
#[command(about = "Ghost identity smoke-test CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print version info.
    Version,
    /// Create a new Ghost identity at the standard path.
    Create {
        /// Optional human-readable display name.
        #[arg(long)]
        display_name: Option<String>,
        /// Optional passphrase. If omitted, the identity file is encrypted with the
        /// OS keystore secret only — convenient but anyone with file + OS access can decrypt.
        #[arg(long)]
        passphrase: Option<String>,
        /// Overwrite an existing identity. DESTRUCTIVE — old identity is unrecoverable.
        #[arg(long)]
        overwrite: bool,
    },
}

fn cmd_create(
    display_name: Option<String>,
    passphrase: Option<String>,
    overwrite: bool,
) -> Result<()> {
    let identity = Identity::create(CreateOptions {
        display_name,
        passphrase: passphrase.as_deref(),
        overwrite,
    })
    .context("create identity")?;

    let path = identity_file().context("resolve identity path")?;
    let id = identity.ghost_id();
    let fp = Fingerprint::of(&id);

    println!("Identity created.");
    println!("  Path:         {}", path.display());
    println!("  Ghost ID:     {}", id);
    println!("  Fingerprint:  {}", fp);
    if let Some(name) = identity.display_name.as_deref() {
        println!("  Display name: {}", name);
    }
    println!(
        "  Pre-keys:     {} one-time + 1 last-resort",
        identity.one_time_prekeys.len()
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("ghost-identity-cli {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Create {
            display_name,
            passphrase,
            overwrite,
        } => cmd_create(display_name, passphrase, overwrite)?,
    }
    Ok(())
}
