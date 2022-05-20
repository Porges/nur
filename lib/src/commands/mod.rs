mod check;
mod init;
mod list;
mod task;

pub use check::Check;
pub use init::Init;
pub use list::List;
pub use task::Task;

use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct Context {
    pub cwd: std::path::PathBuf,
    pub stdout: Sender<String>,
    pub stderr: Sender<String>,
}

#[async_trait::async_trait]
pub trait Command {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()>;
}
