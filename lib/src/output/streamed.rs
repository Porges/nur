use crate::{commands::Message, StatusMessage, TaskResult};

pub struct Streamed {
    pub prefixer: Box<dyn Fn() -> Box<dyn crate::output::Prefixer + Send> + Sync + Send>,
}

#[async_trait::async_trait]
impl crate::output::Output for Streamed {
    async fn handle(
        &self,
        ctx: &crate::commands::Context,
        mut rx: tokio::sync::mpsc::Receiver<crate::StatusMessage>,
    ) {
        let mut prefixer = (self.prefixer)();
        while let Some(msg) = rx.recv().await {
            match msg {
                StatusMessage::StdOut { task_name, line } => {
                    let line = prefixer.prefix(&task_name, line);
                    if ctx.output.send(Message::Out(line)).await.is_err() {
                        break;
                    }
                }
                StatusMessage::StdErr { task_name, line } => {
                    let line = prefixer.prefix(&task_name, line);
                    if ctx.output.send(Message::Err(line)).await.is_err() {
                        break;
                    }
                }
                StatusMessage::TaskStarted { name } => {
                    let msg = format!("— Started task ‘{name}’");
                    let msg = prefixer.prefix(&name, msg);
                    if ctx.output.send(Message::Out(msg)).await.is_err() {
                        break;
                    }
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

                    let msg = prefixer.prefix(&name, msg);
                    if ctx.output.send(Message::Out(msg)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}
