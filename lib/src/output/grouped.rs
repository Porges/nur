use crate::{StatusMessage, TaskStatus};

pub struct Grouped<G> {
    logs: Vec<State>,
    deterministic: bool,
    only_on_failure: bool,
    inner: G,
}

#[derive(Clone)]
enum State {
    Appending(Vec<TaskStatus>),
    ReadyToFlush(Vec<TaskStatus>),
    Flushed,
}

impl<G> Grouped<G> {
    pub fn new(inner: G, task_count: usize, only_on_failure: bool, deterministic: bool) -> Self {
        Grouped {
            inner,
            deterministic,
            only_on_failure,
            logs: vec![State::Appending(Vec::new()); task_count],
        }
    }
}

impl<G: crate::output::Output<StatusMessage>> Grouped<G> {
    fn flush(&mut self, task_id: usize) {
        let state = std::mem::replace(&mut self.logs[task_id], State::Flushed);
        let statuses = match state {
            State::Appending(x) => x,
            State::ReadyToFlush(x) => x,
            State::Flushed => unreachable!("already flushed"),
        };

        for status in statuses {
            self.inner.handle((task_id, status));
        }
    }
}

impl<G: crate::output::Output<StatusMessage>> crate::output::Output<StatusMessage> for Grouped<G> {
    fn handle(&mut self, (task_id, status): crate::StatusMessage) {
        match status {
            TaskStatus::Finished { result } => {
                let state = std::mem::replace(&mut self.logs[task_id], State::Flushed);
                let mut statuses = match state {
                    State::Appending(v) => v,
                    State::ReadyToFlush(v) => v,
                    State::Flushed => unreachable!("already flushed"),
                };

                if self.deterministic {
                    // all previous outputs must be flushed
                    for id in 0..task_id {
                        match &self.logs[id] {
                            State::Appending(_) => {
                                // previous one is still pending,
                                // mark ourselves as ready to flush
                                statuses.push(TaskStatus::Finished { result });
                                let r = std::mem::replace(
                                    &mut self.logs[task_id],
                                    State::ReadyToFlush(statuses),
                                );
                                debug_assert!(matches!(r, State::Flushed));
                                return;
                            }
                            State::ReadyToFlush(_) => {
                                self.flush(id);
                            }
                            State::Flushed => {}
                        }
                    }
                }

                if self.only_on_failure && result.is_ok() {
                    self.inner
                        .handle((task_id, TaskStatus::Finished { result }));
                } else {
                    statuses.push(TaskStatus::Finished { result });
                    for msg in statuses {
                        self.inner.handle((task_id, msg));
                    }
                }
            }
            status => {
                let statuses = match &mut self.logs[task_id] {
                    State::Appending(x) => x,
                    State::ReadyToFlush(x) => x,
                    State::Flushed => unreachable!("already flushed"),
                };

                statuses.push(status);
            }
        }
    }
}
