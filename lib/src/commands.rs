mod check;
mod init;
mod list;
mod task;

use std::io::Write;

pub use check::Check;
pub use init::Init;
pub use list::List;
pub use task::Task;

pub struct Context<'a> {
    pub cwd: std::path::PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

#[derive(Debug)]
pub enum Message {
    Out(String),
    Err(String),
}

pub trait Command {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()>;
}
