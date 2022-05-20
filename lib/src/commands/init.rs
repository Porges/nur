use std::path::Path;

use miette::{Context, IntoDiagnostic};
use question::{Answer, Question};
use tokio::io::AsyncWriteExt;

pub struct Init {
    pub nur_file: Option<std::path::PathBuf>,
    pub dry_run: bool,
}

const DEFAULT_NAME: &str = "nur.yml";

const DEFAULT_CONTENT: &str = r#"version: 1.0
default:
    description: A welcoming message.
    cmds:
    - echo '👋 Hello from your nur file!'
    - echo '💡 Now try `nur --list` to list other tasks you can run.'

more:
    description: 💡 Now run this task with `nur more`…
    cmds:
    - echo '🤖 Running another task… beep boop…'
    - sleep 2
    - echo '💡 You can run `nur --help` to see other available commands,\n   such as --check or --dry-run.'
    - echo
    - sleep 2
    - echo 'This concludes the “tutorial”. Enjoy!'
"#;

#[async_trait::async_trait]
impl crate::commands::Command for Init {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        if let Some(nur_file) = &self.nur_file {
            if nur_file.exists() {
                panic!("nurfile already exists");
            }
        } else {
            match crate::find_nurfile(&ctx.cwd, false) {
                Ok(_) => panic!("nurfile already exists"),
                Err(e) => match e.downcast() {
                    Ok(crate::Error::NurfileNotFound { .. }) => {}
                    Ok(e) => return Err(e.into()),
                    Err(e) => return Err(e),
                },
            }
        }

        let path = self
            .nur_file
            .clone()
            .unwrap_or_else(|| select_location(&ctx.cwd).join(DEFAULT_NAME));

        if self.dry_run {
            if path.exists() {
                panic!("File already exists")
            } else {
                ctx.stdout
                    .send(format!(
                        "[dryrun] Would create file {path:#?} with sample content."
                    ))
                    .await
                    .into_diagnostic()?;
            }
        } else {
            {
                let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .await
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        format!("File already exists at destination ‘{}’", path.display())
                    })?;
                file.write_all(DEFAULT_CONTENT.as_bytes())
                    .await
                    .into_diagnostic()?;
            }

            ctx.stdout
                .send(format!("Created new file {path:#?} with sample content.\n💡 Now try `nur` to run the ‘default’ task."))
                .await
                .into_diagnostic()?;
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
