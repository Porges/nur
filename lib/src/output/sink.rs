use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::commands::Message;

pub struct Sink<'a> {
    pub stdout: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    pub stderr: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    // pub output: mpsc::Sender<Message>,
}

#[async_trait::async_trait]
impl<'a> crate::output::Output<Message> for Sink<'a> {
    async fn handle(&mut self, msg: Message) {
        match msg {
            Message::Out(mut line) => {
                line.push('\n');
                if self.stdout.write_all(line.as_bytes()).await.is_err() {
                    return;
                }
            }
            Message::Err(mut line) => {
                line.push('\n');
                if self.stderr.write_all(line.as_bytes()).await.is_err() {
                    return;
                }
            }
        }
    }
}
