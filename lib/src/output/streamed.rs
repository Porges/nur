use crate::{commands::Message, StatusMessage, TaskResult, TaskStatus};

pub struct Streamed<O> {
    last_id: usize,
    output: O,
    separator: String,
    separator_first: String,
    separator_switch: String,
    separator_last: String,
    names: Vec<String>,
    prefixer: Box<dyn crate::output::Prefixer>,
}

impl<O> Streamed<O> {
    pub fn new(
        output: O,
        separator: String,
        separator_first: String,
        separator_switch: String,
        separator_last: String,
        names: Vec<String>,
        prefixer: Box<dyn crate::output::Prefixer>,
    ) -> Self {
        Streamed {
            last_id: usize::MAX,
            output,
            separator,
            separator_first,
            separator_switch,
            separator_last,
            names,
            prefixer,
        }
    }
}

impl<O: crate::output::Output<Message>> crate::output::Output<StatusMessage> for Streamed<O> {
    fn handle(&mut self, msg: crate::StatusMessage) {
        let (task_id, status) = msg;

        let name = &self.names[task_id];
        let prefix = self.prefixer.prefix(name);
        let sep = if task_id == self.last_id {
            &self.separator
        } else {
            &self.separator_switch
        };

        let to_send = match status {
            TaskStatus::StdOut(line) => {
                let line = format!("{prefix}{}{line}", sep);
                Message::Out(line)
            }
            TaskStatus::StdErr(line) => {
                let line = format!("{prefix}{}{line}", sep);
                Message::Err(line)
            }
            TaskStatus::Started {} => {
                let line = format!("{prefix}{}╴ Started task ‘{name}’", self.separator_first);
                Message::Out(line)
            }
            TaskStatus::Finished { result } => {
                let msg = match result {
                    Ok(TaskResult::Skipped) => {
                        format!("{prefix}{}╴ Task ‘{name}’ skipped", self.separator_last)
                    }
                    Ok(TaskResult::RanToCompletion) => {
                        format!("{prefix}{}╴ Task ‘{name}’ completed", self.separator_last)
                    }
                    Ok(TaskResult::Cancelled) => {
                        format!("{prefix}{}╴ Task ‘{name}’ cancelled", self.separator_last)
                    }
                    Err(r) => format!("{prefix}{}╴ Task ‘{name}’ failed: {r}", self.separator_last),
                };

                Message::Out(msg)
            }
        };

        self.last_id = task_id;

        self.output.handle(to_send);
    }
}
