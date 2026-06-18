#[tokio::main]
async fn main() -> anyhow::Result<()> {
    electronix_id_resolver::run().await
}
