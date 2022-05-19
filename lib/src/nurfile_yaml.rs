use std::collections::BTreeMap;

use miette::IntoDiagnostic;
use serde::Deserialize;
use void::Void;

#[derive(Deserialize)]
pub struct NurYaml {
    version: crate::version::Version,

    tasks: BTreeMap<String, Task>,
}

#[serde_with::serde_as]
#[derive(Deserialize)]
pub struct Task {
    #[serde(alias = "cmds", default)]
    #[serde_as(deserialize_as = "Vec<serde_with::PickFirst<(_, serde_with::DisplayFromStr)>>")]
    commands: Vec<Command>,

    #[serde(alias = "deps", default)]
    dependencies: Vec<String>,

    #[serde(alias = "desc", default)]
    description: String,
}

#[derive(Deserialize, Default)]
pub struct Command {
    #[serde(alias = "cmd")]
    sh: String,

    #[serde(default)]
    ignore_result: bool,
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
            tasks: BTreeMap::from_iter(me.tasks.into_iter().map(|(n, t)| {
                (
                    n,
                    crate::nurfile::Task {
                        description: t.description,
                        commands: t.commands.into_iter().map(|x| x.into()).collect(),
                        dependencies: t.dependencies,
                    },
                )
            })),
        }
    }
}

impl From<Command> for crate::nurfile::Command {
    fn from(c: Command) -> Self {
        crate::nurfile::Command {
            sh: c.sh,
            ignore_result: c.ignore_result,
        }
    }
}

pub fn parse(_: &std::path::Path, input: &str) -> miette::Result<crate::nurfile::NurFile> {
    let nf: NurYaml = serde_yaml::from_str(input).into_diagnostic()?;
    Ok(nf.into())
}
