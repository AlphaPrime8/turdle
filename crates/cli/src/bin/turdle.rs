use anyhow::Error;
use fehler::throws;

#[throws]
#[tokio::main]
async fn main() {
    turdle_cli::start().await?
}
