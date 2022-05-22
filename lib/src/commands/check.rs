use miette::IntoDiagnostic;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::StatusMessage;

pub struct Check {
    pub nur_file: Option<std::path::PathBuf>,
}

#[async_trait::async_trait]
impl crate::commands::Command for Check {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (path, config) = crate::nurfile::load_config(&ctx.cwd, &self.nur_file)?;

        let mut has_error = false;
        for (task_name, task) in config.tasks {
            for (ix, cmd) in task.commands.iter().enumerate() {
                let errors = validate_cmd(&ctx.cwd, &cmd.sh).await?;
                if !errors.is_empty() {
                    has_error = true;
                    let message =
                        format!("Error(s) in task ‘{task_name}’ command {ix}: `{}`", cmd.sh);
                    ctx.tx
                        .send(StatusMessage::StdErr(message))
                        .await
                        .into_diagnostic()?;

                    for line in errors {
                        let line = "\t".to_owned() + &line;
                        ctx.tx
                            .send(StatusMessage::StdErr(line))
                            .await
                            .into_diagnostic()?;
                    }
                }
            }
        }

        if !has_error {
            ctx.tx
                .send(StatusMessage::StdOut(format!(
                    "No syntax errors found in {}",
                    path.display()
                )))
                .await
                .into_diagnostic()?;
        }

        Ok(())
    }
}

async fn validate_cmd(cwd: &std::path::Path, cmd: &str) -> miette::Result<Vec<String>> {
    let proc = tokio::process::Command::new("/bin/sh")
        .args(["-n"])
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .into_diagnostic()?;

    {
        let mut stdin = proc.stdin.expect("stdin handle");
        stdin.write_all(cmd.as_bytes()).await.into_diagnostic()?;
        stdin.flush().await.into_diagnostic()?;
    }

    let mut output = Vec::new();

    let stderr = proc.stderr.expect("stderr handle");
    let mut reader = BufReader::new(stderr).lines();
    while let Some(line) = reader.next_line().await.into_diagnostic()? {
        output.push(line);
    }

    Ok(output)
}
