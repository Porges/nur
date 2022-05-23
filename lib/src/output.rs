use tokio::sync::mpsc::Receiver;

pub mod streamed;
pub mod summarized;

pub use streamed::Streamed;

use crate::nurfile::*;

#[async_trait::async_trait]
pub trait Output: Send {
    async fn handle(&self, ctx: &crate::commands::Context, rx: Receiver<crate::StatusMessage>);
}

pub fn from_config(config: &NurFile, task_name_length_hint: Option<usize>) -> Box<dyn Output> {
    let prefixer: Box<dyn Fn() -> Box<dyn Prefixer + Send> + Sync + Send> =
        match config.options.output.prefix {
            PrefixStyle::NoPrefix => Box::new(|| Box::new(NullPrefixer {})),
            PrefixStyle::Always => Box::new(|| Box::new(AlwaysPrefixer {})),
            PrefixStyle::Aligned => {
                let max_len = task_name_length_hint.unwrap_or_default();
                Box::new(move || {
                    Box::new(AlignedPrefixer {
                        max_len,
                        last: String::new(),
                    })
                })
            }
        };

    Box::new(Streamed { prefixer })
}

pub trait Prefixer {
    fn prefix(&mut self, task_name: &str, s: String) -> String;
}

struct AlwaysPrefixer {}
impl Prefixer for AlwaysPrefixer {
    fn prefix(&mut self, task_name: &str, s: String) -> String {
        task_name.to_string() + " │ " + &s
    }
}

struct AlignedPrefixer {
    max_len: usize,
    last: String,
}
impl Prefixer for AlignedPrefixer {
    fn prefix(&mut self, task_name: &str, s: String) -> String {
        let mut prefix = if self.last == task_name {
            " ".repeat(self.max_len) + " │ "
        } else {
            if task_name.len() > self.max_len {
                self.max_len = task_name.len();
            }

            self.last = task_name.to_owned();
            format!("{task_name:>width$} ┼ ", width = self.max_len)
        };

        prefix += &s;
        prefix
    }
}

struct NullPrefixer {}
impl Prefixer for NullPrefixer {
    fn prefix(&mut self, _task_name: &str, s: String) -> String {
        s
    }
}
