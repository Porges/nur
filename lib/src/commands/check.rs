use miette::IntoDiagnostic;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct Check {
    pub nur_file: Option<std::path::PathBuf>,
}

impl crate::commands::Command for Check {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (path, config) = crate::nurfile::load_config(&ctx.cwd, self.nur_file.as_deref())?;

        let tokio_rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .into_diagnostic()?;

        let mut err_count = 0;
        for (task_name, task) in config.tasks {
            for (ix, cmd) in task.commands.iter().enumerate() {
                let errors = tokio_rt.block_on(validate_cmd(&ctx.cwd, &cmd.sh))?;
                if !errors.is_empty() {
                    err_count += 1;
                    let message = format!(
                        "Error(s) in task ‘{task_name}’ command {ix}: `{}`\n",
                        cmd.sh
                    );

                    ctx.stdout.write_all(message.as_bytes()).into_diagnostic()?;

                    for line in errors {
                        let line = "\t".to_owned() + &line;
                        ctx.stdout.write_all(line.as_bytes()).into_diagnostic()?
                    }
                }
            }
        }

        if err_count == 0 {
            ctx.stdout
                .write_all(format!("No syntax errors found in: {path:#?}\n").as_bytes())
                .into_diagnostic()?;
        } else {
            ctx.stdout
                .write_all(format!("{err_count} syntax errors found in: {path:#?}\n").as_bytes())
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

    {
        let stderr = proc.stderr.expect("stderr handle");
        let mut reader = BufReader::new(stderr).lines();
        while let Some(line) = reader.next_line().await.into_diagnostic()? {
            output.push(line);
        }
    }

    Ok(output)
}
