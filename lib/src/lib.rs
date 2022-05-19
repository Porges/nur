pub mod commands;
pub mod nurfile;
pub mod version;

#[cfg(feature = "kdl")]
pub mod nurfile_kdl;

#[cfg(feature = "yaml")]
pub mod nurfile_yaml;

use std::path::{Path, PathBuf};

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

    #[error("Task ‘{task_name}’ failed when executing command: `{command}`")]
    #[diagnostic(code(nur::task_failed))]
    TaskFailed {
        task_name: String,
        command: String,
        exit_code: Option<i32>,
    },
}

type Result<T> = miette::Result<T, Error>;

pub fn load_config(initial_dir: &Path, file: &Option<&Path>) -> Result<nurfile::NurFile> {
    let (_, nurconfig) = read_nurfile(initial_dir, file)?;
    Ok(nurconfig)
}

type NurfileParser = dyn Fn(&std::path::Path, &str) -> miette::Result<nurfile::NurFile>;

const FORMATS: &[(&str, &NurfileParser)] = &[
    #[cfg(feature = "kdl")]
    ("nur.kdl", &nurfile_kdl::parse),
    #[cfg(feature = "yaml")]
    ("nur.yml", &nurfile_yaml::parse),
    #[cfg(feature = "yaml")]
    ("nur.yaml", &nurfile_yaml::parse),
];

pub fn find_nurfile(
    initial_dir: &Path,
    check_parents: bool,
) -> Result<(PathBuf, &'static NurfileParser)> {
    for root in initial_dir.ancestors() {
        let mut files_to_check: Vec<(PathBuf, &NurfileParser)> = Vec::new();
        for format in FORMATS {
            let file = root.join(format.0);
            if file.exists() {
                files_to_check.push((file, format.1));
            }
        }

        if files_to_check.len() > 1 {
            return Err(Error::MultipleNurFilesFound {
                directory: root.to_owned(),
                files: files_to_check.into_iter().map(|(path, _)| path).collect(),
            });
        }

        if let Some(file) = files_to_check.pop() {
            return Ok(file);
        }

        if !check_parents {
            break;
        }
    }

    // hit top level without finding file:
    Err(Error::NurfileNotFound {
        directory: initial_dir.to_owned(),
    })
}

pub fn read_nurfile(
    initial_dir: &Path,
    file: &Option<&Path>,
) -> Result<(PathBuf, nurfile::NurFile)> {
    let file = match file {
        Some(x) => ((*x).to_owned(), &nurfile_yaml::parse as &NurfileParser),
        None => find_nurfile(initial_dir, true)?,
    };

    let contents = std::fs::read_to_string(&file.0)?;
    let parsed = (file.1)(&file.0, &contents).map_err(|inner| Error::NurfileSyntaxError {
        path: file.0.clone(),
        inner,
    })?;

    Ok((file.0, parsed))
}
