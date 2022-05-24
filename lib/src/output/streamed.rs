use crate::{commands::Message, StatusMessage, TaskResult, TaskStatus};

pub struct Streamed<O> {
    pub output: O,
    pub separator: String,
    pub names: Vec<String>,
    pub prefixer: Box<dyn crate::output::Prefixer + Send + Sync>,
}

#[async_trait::async_trait]
impl<O: crate::output::Output<Message>> crate::output::Output<StatusMessage> for Streamed<O> {
    async fn handle(&mut self, msg: crate::StatusMessage) {
        let (task_id, status) = msg;
        let name = &self.names[task_id];
        let prefix = self.prefixer.prefix(name);

        let to_send = match status {
            TaskStatus::StdOut { line } => {
                let line = format!("{prefix}{}{line}", self.separator);
                Message::Out(line)
            }
            TaskStatus::StdErr { line } => {
                let line = format!("{prefix}{}{line}", self.separator);
                Message::Err(line)
            }
            TaskStatus::Started {} => {
                let line = format!("{prefix}{}— Started task ‘{name}’", self.separator);
                Message::Out(line)
            }
            TaskStatus::Finished { result } => {
                let msg = match result {
                    Ok(TaskResult::Skipped) => format!("— Task ‘{name}’ skipped"),
                    Ok(TaskResult::RanToCompletion) => {
                        format!("— Task ‘{name}’ completed")
                    }
                    Ok(TaskResult::Cancelled) => {
                        format!("— Task ‘{name}’ cancelled")
                    }
                    Err(r) => format!("— Task ‘{name}’ failed: {r}"),
                };

                let line = format!("{prefix}{}{msg}", self.separator);
                Message::Out(line)
            }
        };

        self.output.handle(to_send).await;
    }
}
