#[tokio::main]
async fn main() -> anyhow::Result<()> {
    electronix_id_api::run().await
}
