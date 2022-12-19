#[cfg(feature = "kdl")]
pub mod kdl;

#[cfg(feature = "yaml")]
pub mod yaml;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use miette::IntoDiagnostic;

#[derive(Debug)]
pub struct NurFile {
    pub version: crate::version::Version,

    pub options: Options,

    pub lets: Vec<Let>,

    pub tasks: BTreeMap<String, NurTask>,

    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct Options {
    pub output: OutputOptions,
}

#[derive(Debug, Default)]
pub struct OutputOptions {
    pub style: OutputStyle,
    pub prefix: PrefixStyle,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum OutputStyle {
    Grouped {
        separator: String,
        separator_first: Option<String>,
        separator_last: Option<String>,
        deterministic: bool,
    },
    Streamed {
        separator: String,
        separator_switch: Option<String>,
    },
}

impl Default for OutputStyle {
    fn default() -> Self {
        OutputStyle::Streamed {
            separator: "│".to_string(),
            separator_switch: Some("┼".to_string()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PrefixStyle {
    NoPrefix,
    Always,
    Aligned,
}

impl Default for PrefixStyle {
    fn default() -> Self {
        PrefixStyle::Aligned
    }
}

#[derive(Debug)]
pub struct Let {}

#[derive(Debug, Clone)]
pub struct NurTask {
    pub env: BTreeMap<String, String>,
    pub description: String,
    pub dependencies: Vec<String>,
    pub commands: Vec<NurCommand>,
}

#[derive(Debug, Clone)]
pub struct NurCommand {
    pub env: BTreeMap<String, String>,
    pub sh: String,
    pub ignore_result: bool,
}

pub fn load_config(
    initial_dir: &Path,
    file: &Option<impl AsRef<Path>>,
) -> crate::Result<(PathBuf, NurFile)> {
    let (path, nurconfig) = read_nurfile(initial_dir, file)?;
    Ok((path, nurconfig))
}

type NurfileParser = dyn Fn(&std::path::Path, &str) -> miette::Result<NurFile>;

const FORMATS: &[(&str, &NurfileParser)] = &[
    #[cfg(feature = "kdl")]
    ("nur.kdl", &kdl::parse),
    #[cfg(feature = "yaml")]
    ("nur.yml", &yaml::parse),
    #[cfg(feature = "yaml")]
    ("nur.yaml", &yaml::parse),
];

pub fn find_nurfile(
    initial_dir: &Path,
    check_parents: bool,
) -> crate::Result<(PathBuf, &'static NurfileParser)> {
    for root in initial_dir.ancestors() {
        let mut files_to_check: Vec<(PathBuf, &NurfileParser)> = Vec::new();
        for format in FORMATS {
            let file = root.join(format.0);
            if file.exists() {
                files_to_check.push((file, format.1));
            }
        }

        if files_to_check.len() > 1 {
            return Err(crate::Error::MultipleNurFilesFound {
                directory: root.to_owned(),
                files: files_to_check.into_iter().map(|(path, _)| path).collect(),
            }
            .into());
        }

        if let Some(file) = files_to_check.pop() {
            return Ok(file);
        }

        if !check_parents {
            break;
        }
    }

    // hit top level without finding file:
    Err(crate::Error::NurfileNotFound {
        directory: initial_dir.to_owned(),
    }
    .into())
}

pub fn read_nurfile(
    initial_dir: &Path,
    file: &Option<impl AsRef<Path>>,
) -> crate::Result<(PathBuf, NurFile)> {
    let file = match file {
        Some(x) => (x.as_ref().to_owned(), &yaml::parse as &NurfileParser),
        None => find_nurfile(initial_dir, true)?,
    };

    let contents = std::fs::read_to_string(&file.0).into_diagnostic()?;
    let parsed = (file.1)(&file.0, &contents)?;
    /* TODO: this hides the inner diagnostics
        see: https://github.com/zkat/miette/issues/172
        .map_err(|inner| Error::NurfileSyntaxError {
            path: file.0.clone(),
            inner,
        })?;
    */

    Ok((file.0, parsed))
}
