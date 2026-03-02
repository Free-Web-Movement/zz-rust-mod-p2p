use async_trait::async_trait;
#[async_trait]
pub trait Listener: Sync + 'static {
    async fn run(&mut self) -> anyhow::Result<()>;
}
