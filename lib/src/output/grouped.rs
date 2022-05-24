use std::collections::BTreeMap;

use crate::StatusMessage;

pub struct Grouped<G> {
    msgs: BTreeMap<String, Vec<StatusMessage>>,
    inner: G,
}

impl<G> Grouped<G> {
    pub fn new(inner: G) -> Self {
        Grouped {
            inner,
            msgs: BTreeMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl<G: crate::output::Output<StatusMessage> + Send + Sync> crate::output::Output<StatusMessage>
    for Grouped<G>
{
    async fn handle(&mut self, msg: crate::StatusMessage) {
        match msg {
            StatusMessage::StdOut { task_name, line } => {
                self.msgs
                    .entry(task_name.clone())
                    .or_default()
                    .push(StatusMessage::StdOut { task_name, line });
            }
            StatusMessage::StdErr { task_name, line } => {
                self.msgs
                    .entry(task_name.clone())
                    .or_default()
                    .push(StatusMessage::StdErr { task_name, line });
            }
            StatusMessage::TaskStarted { name } => {
                self.msgs
                    .entry(name.clone())
                    .or_default()
                    .push(StatusMessage::TaskStarted { name });
            }
            StatusMessage::TaskFinished { name, result } => {
                if let Some(msgs) = self.msgs.remove(&name) {
                    for msg in msgs {
                        self.inner.handle(msg).await;
                    }
                }

                self.inner
                    .handle(StatusMessage::TaskFinished { name, result })
                    .await;
            }
        }
    }
}
