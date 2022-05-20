use std::{collections::BTreeMap, error::Error};

use miette::Diagnostic;
use serde::Deserialize;
use void::Void;

#[derive(Deserialize)]
pub struct NurYaml {
    version: crate::version::Version,

    #[serde(default)]
    shared: Shared,

    #[serde(flatten)]
    tasks: BTreeMap<String, Task>,
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
    #[serde(alias = "cmds", default)]
    #[serde_as(deserialize_as = "Vec<serde_with::PickFirst<(_, serde_with::DisplayFromStr)>>")]
    commands: Vec<Command>,

    #[serde(alias = "deps", default)]
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
            lets: vec![],
            env: me.shared.environment,
            tasks: BTreeMap::from_iter(me.tasks.into_iter().map(|(n, t)| {
                (
                    n,
                    crate::nurfile::NurTask {
                        env: t.environment,
                        description: t.description,
                        commands: t.commands.into_iter().map(|x| x.into()).collect(),
                        dependencies: t.dependencies,
                    },
                )
            })),
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
