use std::fmt::Write;

use tokio::io::AsyncWrite;

pub mod grouped;
pub mod sink;
pub mod streamed;

pub use grouped::Grouped;
pub use streamed::Streamed;

use crate::nurfile::*;

#[async_trait::async_trait]
pub trait Output<T>: Send + Sync {
    async fn handle(&mut self, msg: T);
}

pub fn create<'a>(
    stdout: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    stderr: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    options: &OutputOptions,
    execution_order: &[&str],
) -> Box<dyn Output<crate::StatusMessage> + 'a> {
    let task_name_length_hint = execution_order.iter().map(|x| x.len()).max();

    let prefixer: Box<dyn Prefixer + Send + Sync> = match options.prefix {
        PrefixStyle::NoPrefix => Box::new(NullPrefixer {}),
        PrefixStyle::Always => Box::new(AlwaysPrefixer {}),
        PrefixStyle::Aligned => {
            let max_len = task_name_length_hint.unwrap_or_default();
            Box::new(AlignedPrefixer::new(max_len))
        }
    };

    let output = sink::Sink { stdout, stderr };
    let streamed = |sep: &str| Streamed {
        output,
        separator: sep.to_string(),
        prefixer,
        names: execution_order.iter().map(|x| x.to_string()).collect(),
    };

    match &options.style {
        OutputStyle::Grouped {
            separator,
            separator_start,
            separator_end,
            deterministic,
        } => Box::new(Grouped::new(
            streamed(separator),
            execution_order.len(),
            *deterministic,
        )),
        OutputStyle::Streamed { separator } => Box::new(streamed(separator)),
    }
}

pub trait Prefixer {
    fn prefix<'a: 's, 's>(&'s mut self, task_name: &'a str) -> &'s str;
}

struct AlwaysPrefixer {}
impl Prefixer for AlwaysPrefixer {
    fn prefix<'a: 's, 's>(&mut self, task_name: &'a str) -> &'s str {
        task_name
    }
}

struct AlignedPrefixer {
    max_len: usize,
    last: String,

    first_prefix: String,
    prefix: String,
}

impl AlignedPrefixer {
    pub fn new(max_len: usize) -> Self {
        AlignedPrefixer {
            max_len,
            prefix: " ".repeat(max_len),

            last: String::new(),
            first_prefix: String::new(),
        }
    }
}

impl Prefixer for AlignedPrefixer {
    fn prefix<'a: 's, 's>(&'s mut self, task_name: &'a str) -> &'s str {
        if self.last == task_name {
            &self.prefix
        } else {
            if task_name.len() > self.max_len {
                self.max_len = task_name.len();
                self.prefix = " ".repeat(self.max_len);
            }

            self.last = task_name.to_owned();

            self.first_prefix.clear();
            write!(
                self.first_prefix,
                "{task_name:>width$}",
                width = self.max_len
            )
            .expect("write should always succeed");

            // TODO: restore ┼ behaviour

            &self.first_prefix
        }
    }
}

const EMPTY_STR: &str = "";
struct NullPrefixer {}
impl Prefixer for NullPrefixer {
    fn prefix<'a: 's, 's>(&'s mut self, _task_name: &'a str) -> &'s str {
        EMPTY_STR
    }
}
