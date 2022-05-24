use crate::{StatusMessage, TaskStatus};

pub struct Grouped<G> {
    logs: Vec<State>,
    deterministic: bool,
    inner: G,
}

#[derive(Clone)]
enum State {
    Appending(Vec<TaskStatus>),
    ReadyToFlush(Vec<TaskStatus>),
    Flushed,
}

impl<G> Grouped<G> {
    pub fn new(inner: G, task_count: usize, deterministic: bool) -> Self {
        Grouped {
            inner,
            deterministic,
            logs: vec![State::Appending(Vec::new()); task_count],
        }
    }
}

impl<G: crate::output::Output<StatusMessage> + Send + Sync> Grouped<G> {
    async fn flush(&mut self, task_id: usize) {
        let state = std::mem::replace(&mut self.logs[task_id], State::Flushed);

        let msgs = match state {
            State::Appending(x) => x,
            State::ReadyToFlush(x) => x,
            State::Flushed => {
                debug_assert!(false, "already flushed");
                return;
            }
        };

        for msg in msgs {
            self.inner.handle((task_id, msg)).await;
        }
    }
}

#[async_trait::async_trait]
impl<G: crate::output::Output<StatusMessage> + Send + Sync> crate::output::Output<StatusMessage>
    for Grouped<G>
{
    async fn handle(&mut self, msg: crate::StatusMessage) {
        let (task_id, status) = msg;

        let mut push = |msg| {
            let msgs = match &mut self.logs[task_id] {
                State::Appending(x) => x,
                State::ReadyToFlush(x) => x,
                State::Flushed => {
                    debug_assert!(false, "already flushed");
                    return;
                }
            };

            msgs.push(msg);
        };

        match status {
            it @ TaskStatus::Finished { .. } => {
                let state = std::mem::replace(&mut self.logs[task_id], State::Flushed);
                let mut vec = match state {
                    State::Appending(v) => v,
                    State::ReadyToFlush(v) => v,
                    State::Flushed => {
                        debug_assert!(false, "already flushed");
                        return;
                    }
                };

                vec.push(it);

                if self.deterministic {
                    // all previous outputs must be flushed
                    let mut id = 0;
                    while id < task_id {
                        match &self.logs[id] {
                            State::Appending(_) => {
                                // previous one is still pending,
                                // mark ourselves as ready to flush
                                let r = std::mem::replace(
                                    &mut self.logs[task_id],
                                    State::ReadyToFlush(vec),
                                );
                                debug_assert!(matches!(r, State::Flushed));
                                return;
                            }
                            State::ReadyToFlush(_) => {
                                self.flush(id).await;
                            }
                            State::Flushed => {}
                        }

                        id += 1;
                    }
                }

                for msg in vec {
                    self.inner.handle((task_id, msg)).await;
                }
            }
            other => {
                push(other);
            }
        }
    }
}
