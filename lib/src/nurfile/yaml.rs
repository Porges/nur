use std::{collections::BTreeMap, path::Path};

use miette::Diagnostic;
use serde::Deserialize;
use thiserror::Error;
use void::Void;

#[derive(Deserialize)]
pub struct NurYaml {
    version: crate::version::Version,

    #[serde(default)]
    options: Options,

    #[serde(default)]
    shared: Shared,

    #[serde(flatten)]
    tasks: BTreeMap<String, Task>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    output: Option<OutputOptions>,
}

#[serde_with::serde_as]
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct OutputOptions {
    #[serde(default)]
    prefix: Option<Prefix>,

    #[serde(default)]
    #[serde_as(
        deserialize_as = "Option<serde_with::PickFirst<(_, serde_with::FromInto<OutputStyleAliases>)>>"
    )]
    style: Option<OutputStyle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyle {
    Grouped {
        separator: Option<String>,
        separator_start: Option<String>,
        separator_end: Option<String>,
        #[serde(default)]
        deterministic: bool,
    },
    Streamed {
        separator: Option<String>,
        separator_switch: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyleAliases {
    Streamed,
    Grouped,
}

impl From<OutputStyleAliases> for OutputStyle {
    fn from(osa: OutputStyleAliases) -> Self {
        match osa {
            OutputStyleAliases::Streamed => OutputStyle::Streamed {
                separator: None,
                separator_switch: None,
            },
            OutputStyleAliases::Grouped => OutputStyle::Grouped {
                separator: None,
                separator_start: None,
                separator_end: None,
                deterministic: false,
            },
        }
    }
}

#[derive(Deserialize)]
enum Prefix {
    None,
    Always,
    Aligned,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Shared {
    #[serde(alias = "env", default)]
    environment: BTreeMap<String, String>,
}

#[serde_with::serde_as]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    #[serde(default)]
    #[serde_as(
        deserialize_as = "serde_with::OneOrMany<serde_with::PickFirst<(_, serde_with::DisplayFromStr)>>"
    )]
    run: Vec<Command>,

    #[serde(alias = "after", default)]
    #[serde_as(deserialize_as = "serde_with::OneOrMany<_>")]
    dependencies: Vec<String>,

    #[serde(alias = "desc", default)]
    description: String,

    #[serde(alias = "env", default)]
    environment: BTreeMap<String, String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[serde(alias = "cmd")]
    sh: String,

    #[serde(default)]
    ignore_result: bool,

    #[serde(alias = "env", default)]
    environment: BTreeMap<String, String>,
}

impl std::str::FromStr for Command {
    type Err = Void;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Command {
            sh: s.to_string(),
            ..Default::default()
        })
    }
}

impl From<NurYaml> for crate::nurfile::NurFile {
    fn from(me: NurYaml) -> Self {
        crate::nurfile::NurFile {
            version: me.version,
            options: me.options.into(),
            lets: vec![],
            env: me.shared.environment,
            tasks: BTreeMap::from_iter(me.tasks.into_iter().map(|(n, t)| {
                (
                    n,
                    crate::nurfile::NurTask {
                        env: t.environment,
                        description: t.description,
                        commands: t.run.into_iter().map(|x| x.into()).collect(),
                        dependencies: t.dependencies,
                    },
                )
            })),
        }
    }
}

impl From<Options> for crate::nurfile::Options {
    fn from(o: Options) -> Self {
        crate::nurfile::Options {
            output: o.output.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<OutputOptions> for crate::nurfile::OutputOptions {
    fn from(o: OutputOptions) -> Self {
        crate::nurfile::OutputOptions {
            style: o.style.map(Into::into).unwrap_or_default(),
            prefix: o.prefix.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<OutputStyle> for crate::nurfile::OutputStyle {
    fn from(o: OutputStyle) -> Self {
        match o {
            OutputStyle::Grouped {
                separator,
                separator_end,
                separator_start,
                deterministic,
            } => crate::nurfile::OutputStyle::Grouped {
                separator: separator.unwrap_or_else(|| "│".to_string()),
                separator_first: Some(separator_start.unwrap_or_else(|| "╭".to_string())),
                separator_last: Some(separator_end.unwrap_or_else(|| "╰".to_string())),
                deterministic,
            },
            OutputStyle::Streamed {
                separator,
                separator_switch,
            } => crate::nurfile::OutputStyle::Streamed {
                separator: separator.unwrap_or_else(|| "│".to_string()),
                separator_switch: Some(separator_switch.unwrap_or_else(|| "┼".to_string())),
            },
        }
    }
}

impl From<Prefix> for crate::nurfile::PrefixStyle {
    fn from(p: Prefix) -> Self {
        match p {
            Prefix::None => crate::nurfile::PrefixStyle::NoPrefix,
            Prefix::Always => crate::nurfile::PrefixStyle::Always,
            Prefix::Aligned => crate::nurfile::PrefixStyle::Aligned,
        }
    }
}

impl From<Command> for crate::nurfile::NurCommand {
    fn from(c: Command) -> Self {
        crate::nurfile::NurCommand {
            env: c.environment,
            sh: c.sh,
            ignore_result: c.ignore_result,
        }
    }
}

pub fn parse(path: &Path, input: &str) -> miette::Result<crate::nurfile::NurFile> {
    let nf: NurYaml = serde_yaml::from_str(input).map_err(|e| translate_error(path, e, input))?;
    Ok(nf.into())
}

// Present a serde_yaml::Error as a miette::Diagnostic
#[derive(Debug, Error, Diagnostic)]
#[error("YAML error: {inner}")]
struct YamlError {
    inner: serde_yaml::Error,

    #[source_code]
    src: miette::NamedSource,

    #[label("here")]
    err_span: Option<miette::SourceSpan>,
}

fn translate_error(path: &Path, e: serde_yaml::Error, input: &str) -> miette::Report {
    YamlError {
        src: miette::NamedSource::new(path.to_string_lossy(), input.to_string()),
        err_span: e
            .location()
            .map(|loc| miette::SourceSpan::new(loc.index().into(), 0.into())),
        inner: e,
    }
    .into()
}
