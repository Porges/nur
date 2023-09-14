use std::io::Write;

use crate::commands::Message;

pub struct Sink<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

impl<'a> crate::output::Output<Message> for Sink<'a> {
    fn handle(&mut self, msg: Message) {
        match msg {
            Message::Out(mut line) => {
                line.push('\n');
                _ = self.stdout.write_all(line.as_bytes());
            }
            Message::Err(mut line) => {
                line.push('\n');
                _ = self.stderr.write_all(line.as_bytes());
            }
        }
    }
}
