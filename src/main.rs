use simprint_runtime::app::RuntimeBootstrap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    RuntimeBootstrap::stdio()?.run().await?;
    Ok(())
}
