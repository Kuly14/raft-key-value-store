use std::{net::SocketAddr, str::FromStr};

use anyhow::Result;
use clap::{Parser, Subcommand};
use raft::Config;

const NUM_OF_NODES: u32 = 3;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Node {
        #[clap(short, long)]
        id: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = match cli.command {
        Commands::Node { id } => Config::new(
            id,
            NUM_OF_NODES,
            SocketAddr::from_str(format!("127.0.0.1:{}", 8000 + id).as_str()).unwrap(),
        ),
    };

    println!("{:#?}", config);
    raft::run(config).await?;
    Ok(())
}
