use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use clap::Parser;
use miette::IntoDiagnostic;

use nur_lib::commands;

/// A robust task runner.
#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Names of the tasks in the Nurfile to run.
    #[clap(exclusive = true)]
    task_names: Vec<String>,

    /// Initialize a fresh Nurfile.
    #[clap(exclusive = true, long)]
    init: bool,

    /// List all the tasks available in the Nurfile.
    #[clap(exclusive = true, long, short)]
    list: bool,

    #[clap(long)]
    nur_file: Option<PathBuf>,
}

fn build_command(cli: Cli, cwd: &Path) -> Box<dyn commands::Command> {
    if cli.init {
        if !matches!(
            nur_lib::find_nurfile(cwd, false),
            Err(nur_lib::Error::NurfileNotFound { .. })
        ) {
            panic!("nurfile already exists");
        }

        return Box::new(commands::Init {});
    }

    if cli.list {
        return Box::new(commands::List {});
    }

    Box::new(commands::Task {
        task_names: cli.task_names,
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let cwd = std::env::current_dir().into_diagnostic()?;
    let config = nur_lib::load_config(&cwd, &cli.nur_file.as_deref())?;
    let command = build_command(cli, &cwd);

    let (tx_std, mut rx_std) = tokio::sync::mpsc::channel::<String>(1000);
    let (tx_err, mut rx_err) = tokio::sync::mpsc::channel::<String>(1000);

    let stdout_writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(line) = rx_std.recv().await {
            if stdout.write(line.as_bytes()).await.is_err() {
                break;
            }

            if stdout.write(b"\n").await.is_err() {
                break;
            }
        }
    });

    let stderr_writer = tokio::spawn(async move {
        let mut stderr = tokio::io::stderr();
        while let Some(line) = rx_err.recv().await {
            if stderr.write(line.as_bytes()).await.is_err() {
                break;
            }

            if stderr.write(b"\n").await.is_err() {
                break;
            }
        }
    });

    let ctx = nur_lib::commands::Context {
        cwd,
        stdout: tx_std,
        stderr: tx_err,
    };

    command.run(ctx, config).await?;
    stdout_writer.await.into_diagnostic()?;
    stderr_writer.await.into_diagnostic()?;

    #[cfg(feature = "nu")]
    {
        let engine_state = nu_command::create_default_context(&cwd);
        let stack = {
            let mut it = nu_protocol::engine::Stack::new();
            it.add_env_var(
                "PWD".to_string(),
                nu_protocol::Value::String {
                    val: cwd.to_string_lossy().to_string(),
                    span: nu_protocol::Span::test_data(),
                },
            );
            it
        };

        for (_name, task) in &config.tasks {
            let mut stack = stack.clone(); // share vars across commands in the same task
            for cmd in &task.commands {
                let (lexed, err) = nu_parser::lex(cmd.as_bytes(), 0, &[], &[], false);
                if let Some(err) = err {
                    return Err(err.into());
                }

                let (lite_block, err) = nu_parser::lite_parse(&lexed);
                if let Some(err) = err {
                    return Err(err.into());
                }

                let mut working_set = nu_protocol::engine::StateWorkingSet::new(&engine_state);
                working_set.add_file("command".into(), cmd.as_bytes());
                let (block, err) =
                    nu_parser::parse_block(&mut working_set, &lite_block, true, &[], false);
                if let Some(err) = err {
                    return Err(err.into());
                }

                let input = nu_protocol::PipelineData::Value(
                    nu_protocol::Value::Nothing {
                        span: nu_protocol::Span::new(0, 0),
                    },
                    None,
                );

                let result =
                    nu_engine::eval_block(&engine_state, &mut stack, &block, input, false, false)?;

                result.print(&engine_state, &mut stack)?;
            }
        }
    }

    Ok(())
}
