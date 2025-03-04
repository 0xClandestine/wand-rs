use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Finds dead/unused code within a Solidity project.
    Vacuum(commands::vacuum::VacuumArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Vacuum(args) => commands::vacuum::run(args)?,
    }

    Ok(())
}
