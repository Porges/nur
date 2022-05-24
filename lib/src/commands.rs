mod check;
mod init;
mod list;
mod task;

pub use check::Check;
pub use init::Init;
pub use list::List;
pub use task::Task;
use tokio::io::AsyncWrite;

pub struct Context<'a> {
    pub cwd: std::path::PathBuf,
    pub stdout: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    pub stderr: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
}

#[derive(Debug)]
pub enum Message {
    Out(String),
    Err(String),
}

#[async_trait::async_trait]
pub trait Command {
    async fn run<'a>(&self, ctx: crate::commands::Context<'a>) -> miette::Result<()>;
}
