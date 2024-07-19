pub mod commands;
pub mod nurfile;
pub mod output;
pub mod version;

use std::{fmt::Display, path::PathBuf};

use miette::Diagnostic;
use thiserror::Error;
use version::{ParseVersionError, Version};

pub const CURRENT_FILE_VERSION: Version = Version { major: 0, minor: 1 };

#[derive(Error, Diagnostic, Debug)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(code(nur::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Missing version in nurfile")]
    #[diagnostic(code(nur::missing_version))]
    MissingVersion,

    #[error("Invalid version in nurfile")]
    #[diagnostic(code(nur::invalid_version))]
    InvalidVersion(ParseVersionError),

    #[error("Unsupported version {version} in nurfile")]
    #[diagnostic(code(nur::unsupported_version))]
    UnsupportedVersion { version: Version },

    #[error("Internal error")]
    #[diagnostic(code(nur::internal_error))]
    InternalError(#[from] Box<dyn std::error::Error + Sync + Send>),

    #[error("Nur file not found in ‘{directory}’, or any of its parent directories")]
    #[diagnostic(
        code(nur::no_nur_files),
        help("try creating a nurfile with ‘nur --init’")
    )]
    NurfileNotFound { directory: PathBuf },

    #[error("Nur file already exists at destination: {path:?}")]
    #[diagnostic(code(nur::nur_file_already_exists))]
    NurfileAlreadyExists { path: PathBuf },

    #[error("Multiple Nur files found in ‘{directory}’")]
    #[diagnostic(
        code(nur::multiple_nur_files),
        help("there should only be one nurfile per directory")
    )]
    MultipleNurFilesFound {
        directory: PathBuf,
        files: Vec<PathBuf>,
    },

    #[error("Nur file {path:?} has a syntax error")]
    #[diagnostic(code(nur::syntax_error))]
    NurfileSyntaxError {
        path: PathBuf,
        #[diagnostic_source]
        inner: miette::Report,
    },

    #[error("Nur file {path:?} has a task cycle: {cycle}")]
    #[diagnostic(code(nur::task_cycle))]
    TaskCycle { path: PathBuf, cycle: Cycle },

    #[error("Unknown task ‘{task_name}’")]
    #[diagnostic(
        code(nur::no_such_task),
        help("to see a list of available tasks, run `nur --list`")
    )]
    NoSuchTask { task_name: String },

    #[error("Task ‘{task_name}’ failed")]
    #[diagnostic(code(nur::task_failed))]
    TaskFailed {
        task_name: String,
        #[source]
        #[diagnostic_source]
        task_error: TaskError,
    },

    #[error("Multiple failures")]
    #[diagnostic(code(nur::multiple_failures))]
    Multiple {
        #[related]
        failures: Vec<Error>,
    },
}

pub(crate) fn internal_error(e: impl std::error::Error + Sync + Send + 'static) -> Error {
    Error::InternalError(Box::new(e))
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cycle {
    path: Vec<String>,
}

impl Display for Cycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.path {
            write!(f, "{} → ", item)?;
        }

        write!(f, "{}", self.path[0])
    }
}

type Result<T> = miette::Result<T, Error>;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum TaskResult {
    Skipped,
    Cancelled,
    RanToCompletion,
}

#[derive(Error, Debug, Diagnostic, Clone)]
pub enum TaskError {
    #[error("shell command `{command}` failed ({exit_status})")]
    #[diagnostic(code(nur::shell_command_failed))]
    Failed {
        command: String,
        exit_status: std::process::ExitStatus,
    },

    #[error("error starting executable ‘{executable}’: {kind}")]
    #[diagnostic(code(nur::executable_start_error))]
    ExecutableError {
        executable: String,
        kind: std::io::ErrorKind,
    },

    #[error("error waiting for command to complete: {kind}")]
    #[diagnostic(code(nur::executable_wait_error))]
    ExecutableWaitFailure {
        executable: String,
        kind: std::io::ErrorKind,
    },
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    StdOut(String),
    StdErr(String),
    Started {},
    Finished {
        result: std::result::Result<TaskResult, TaskError>,
    },
}

pub type StatusMessage = (usize, TaskStatus);
