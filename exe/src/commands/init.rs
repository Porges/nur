use miette::{Context, IntoDiagnostic};
use question::{Answer, Question};
use std::{io::Write, path::Path};

use nur_lib::nurfile::{NurFile, Task};

pub fn run(cwd: &Path) -> miette::Result<()> {
    let sample_file: NurFile = NurFile {
        version: nur_lib::CURRENT_FILE_VERSION,
        lets: Default::default(),
        tasks: std::collections::BTreeMap::from([(
            "hello".to_string(),
            Task {
                description: "".to_string(),
                commands: vec!["echo 'Hello, world!'".to_string()],
                dependencies: Default::default(),
            },
        )]),
    };

    let yaml_config: nur_lib::nurfile_yaml::NurYaml = sample_file.into();
    let content = serde_yaml::to_string(&yaml_config).into_diagnostic()?;

    let location = select_location(cwd);
    let path = location.join("nur.yml");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("File already exists at destination ‘{}’", path.display()))?;

    file.write_all(content.as_bytes()).into_diagnostic()?;

    Ok(())
}

fn select_location(cwd: &Path) -> &Path {
    if let Some(repo_location) = find_git_dir(cwd) {
        let create_at_root = Question::new("It looks like you are in a Git repository. Would you like to create the Nurfile at the root?")
            .default(Answer::YES)
            .show_defaults()
            .confirm();

        if create_at_root == Answer::YES {
            return repo_location;
        }
    }

    cwd
}

fn find_git_dir(cwd: &std::path::Path) -> Option<&Path> {
    for path in cwd.ancestors().skip(1) {
        let git_dir = path.join(".git");
        if git_dir.exists() && git_dir.is_dir() {
            return Some(path);
        }
    }

    None
}
