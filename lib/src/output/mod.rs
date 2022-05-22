use tokio::sync::mpsc::Receiver;

pub mod streamed;
pub mod summarized;

pub use streamed::Streamed;

#[async_trait::async_trait]
pub trait Output: Send {
    async fn handle(&self, ctx: &crate::commands::Context, rx: Receiver<crate::StatusMessage>);
}

pub fn from_config(_config: &crate::nurfile::NurFile) -> Box<dyn Output> {
    Box::new(Streamed {})
}
