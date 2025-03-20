mod net;

pub use net::config::Config;

use anyhow::Result;
use net::{manager::NetworkManager, swarm::Swarm};
use tokio::signal::ctrl_c;

pub async fn run(config: Config) -> Result<()> {
    tracing_subscriber::fmt::init();
    let swarm = Swarm::new(config).await?;
    let manager = NetworkManager::new(swarm);
    tokio::spawn(manager);

    let _ = ctrl_c().await;
    Ok(())
}
