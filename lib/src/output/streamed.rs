use crate::{commands::Message, StatusMessage, TaskResult};

pub struct Streamed<O> {
    pub output: O,
    pub separator: String,
    pub prefixer: Box<dyn crate::output::Prefixer + Send + Sync>,
}

#[async_trait::async_trait]
impl<O: crate::output::Output<Message>> crate::output::Output<StatusMessage> for Streamed<O> {
    async fn handle(&mut self, msg: crate::StatusMessage) {
        let to_send = match msg {
            StatusMessage::StdOut { task_name, line } => {
                let prefix = self.prefixer.prefix(&task_name);
                let line = format!("{prefix} {} {line}", self.separator);
                Message::Out(line)
            }
            StatusMessage::StdErr { task_name, line } => {
                let prefix = self.prefixer.prefix(&task_name);
                let line = format!("{prefix} {} {line}", self.separator);
                Message::Err(line)
            }
            StatusMessage::TaskStarted { name } => {
                let prefix = self.prefixer.prefix(&name);
                let line = format!("{prefix} {} — Started task ‘{name}’", self.separator);
                Message::Out(line)
            }
            StatusMessage::TaskFinished { name, result } => {
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

                let prefix = self.prefixer.prefix(&name);
                let line = format!("{prefix} {} {msg}", self.separator);
                Message::Out(line)
            }
        };

        self.output.handle(to_send).await;
    }
}
