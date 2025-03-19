use anyhow::Result;
use raft::Config;

const NUM_OF_NODES: u32 = 3;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: Parse cli args for the id
    let id = 1;
    let config = Config::new(id, NUM_OF_NODES);
    raft::run(config).await?;
    Ok(())
}
