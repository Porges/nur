use std::{io::Write, path::Path};

use miette::IntoDiagnostic;
use question::{Answer, Question};

pub struct Init {
    pub nur_file: Option<std::path::PathBuf>,
    pub dry_run: bool,
}

const DEFAULT_NAME: &str = "nur.yml";

const DEFAULT_CONTENT: &str = r"version: 1.0
default:
    description: A welcoming message.
    run:
    - echo '👋 Hello from your nur file!'
    - echo '💡 Now try `nur --list` to list other tasks you can run.'

more:
    description: 💡 Now run this task with `nur more`…
    run:
    - echo '🤖 Running another task… beep boop…'
    - sleep 2
    - echo '💡 You can run `nur --help` to see other available commands,\n   such as --check or --dry-run.'
    - echo
    - sleep 2
    - echo 'This concludes the “tutorial”. Enjoy!'
";

impl crate::commands::Command for Init {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        // ensure that we aren’t going to overwrite anything:
        if let Some(nur_file) = &self.nur_file {
            if nur_file.exists() {
                return Err(crate::Error::NurfileAlreadyExists {
                    path: nur_file.clone(),
                }
                .into());
            }
        } else {
            match crate::nurfile::find_nurfile(&ctx.cwd, false) {
                Ok((path, _)) => return Err(crate::Error::NurfileAlreadyExists { path }.into()),
                Err(e) => match e {
                    crate::Error::NurfileNotFound { .. } => {
                        // note that this is the only success case
                    }
                    e => return Err(e.into()),
                },
            }
        }

        let path = self
            .nur_file
            .clone()
            .unwrap_or_else(|| select_location(&ctx.cwd).join(DEFAULT_NAME));

        if self.dry_run {
            if path.exists() {
                return Err(crate::Error::NurfileAlreadyExists { path }.into());
            } else {
                let msg = format!("[dryrun] Would create file {path:#?} with sample content.\n");
                ctx.stdout.write_all(msg.as_bytes()).into_diagnostic()?;
            }
        } else {
            {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| {
                        if e.kind() == std::io::ErrorKind::AlreadyExists {
                            crate::Error::NurfileAlreadyExists { path: path.clone() }
                        } else {
                            crate::Error::IoError(e)
                        }
                    })?;

                file.write_all(DEFAULT_CONTENT.as_bytes())
                    .into_diagnostic()?;
            }

            let msg = format!("Created new file {path:#?} with sample content.\n💡 Now try `nur` to run the ‘default’ task.\n");
            ctx.stdout.write_all(msg.as_bytes()).into_diagnostic()?;
        }

        Ok(())
    }
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
