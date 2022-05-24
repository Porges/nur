use std::{collections::BTreeMap, error::Error};

use miette::Diagnostic;
use serde::Deserialize;
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
#[serde(tag = "style")]
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
    },
}

#[derive(Deserialize)]
pub enum OutputStyleAliases {
    Streamed,
    Grouped,
}

impl From<OutputStyleAliases> for OutputStyle {
    fn from(osa: OutputStyleAliases) -> Self {
        match osa {
            OutputStyleAliases::Streamed => OutputStyle::Streamed { separator: None },
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
                separator_end,
                separator_start,
                deterministic,
            },
            OutputStyle::Streamed { separator } => crate::nurfile::OutputStyle::Streamed {
                separator: separator.unwrap_or_else(|| "│".to_string()),
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

pub fn parse(_: &std::path::Path, input: &str) -> miette::Result<crate::nurfile::NurFile> {
    let nf: NurYaml = serde_yaml::from_str(input).map_err(|e| translate_error(e, input))?;
    Ok(nf.into())
}

struct WrapErr {
    e: serde_yaml::Error,
    source_code: String,
}

impl std::fmt::Display for WrapErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.e.fmt(f)
    }
}

impl std::fmt::Debug for WrapErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.e, f)
    }
}

impl std::error::Error for WrapErr {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.e.source()
    }
}

impl miette::Diagnostic for WrapErr {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        None
    }

    fn severity(&self) -> Option<miette::Severity> {
        None
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        None
    }

    fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        None
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source_code)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.e.location().map(|loc| {
            let label = miette::LabeledSpan::new(Some("here".to_string()), loc.index(), 0);
            Box::new([label].into_iter()) as Box<dyn Iterator<Item = _>>
        })
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        None
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        None
    }
}

fn translate_error(e: serde_yaml::Error, input: &str) -> miette::Report {
    miette::Report::new(WrapErr {
        e,
        source_code: input.to_string(),
    })
}
