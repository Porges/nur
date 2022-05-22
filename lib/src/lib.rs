pub mod commands;
pub mod nurfile;
pub mod version;

use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;
use version::Version;

pub const CURRENT_FILE_VERSION: Version = Version { major: 0, minor: 1 };

#[derive(Error, Diagnostic, Debug)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(code(nur::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Nurfile not found in ‘{directory}’, or any of its parent directories")]
    #[diagnostic(
        code(nur::no_nur_files),
        help("try creating a nurfile with ‘nur --init’")
    )]
    NurfileNotFound { directory: PathBuf },

    #[error("Multiple nurfiles found in ‘{directory}’")]
    #[diagnostic(
        code(nur::multiple_nur_files),
        help("there should only be one nurfile per directory")
    )]
    MultipleNurFilesFound {
        directory: PathBuf,
        files: Vec<PathBuf>,
    },

    #[error("Nurfile had a syntax error")]
    #[diagnostic(code(nur::syntax_error))]
    NurfileSyntaxError {
        path: PathBuf,
        #[diagnostic_source]
        inner: miette::Report,
    },

    #[error("Nurfile has a task cycle involving task ‘{task_name}’")]
    #[diagnostic(code(nur::task_cycle))]
    TaskCycle { task_name: String },

    #[error("Unknown task ‘{task_name}’")]
    #[diagnostic(
        code(nur::no_such_task),
        help("to see a list of available tasks, run `nur --list`")
    )]
    NoSuchTask { task_name: String },

    #[error("Task ‘{task_name}’ failed: {task_error}")]
    #[diagnostic(code(nur::task_failed))]
    TaskFailed {
        task_name: String,
        task_error: TaskError,
    },
}

type Result<T> = miette::Result<T>;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum TaskResult {
    Skipped,
    Cancelled,
    RanToCompletion,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum TaskError {
    #[error("external command failed ({exit_status}) when executing: `{command}`")]
    Failed {
        command: String,
        exit_status: std::process::ExitStatus,
    },
    #[error("I/O error: {kind}")]
    IoError { kind: std::io::ErrorKind },
}

#[derive(Debug)]
pub enum StatusMessage {
    StdOut(String),
    StdErr(String),
    TaskStarted {
        name: String,
    },
    TaskFinished {
        name: String,
        result: std::result::Result<TaskResult, TaskError>,
    },
}
