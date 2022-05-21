use clap::Parser;
use miette::IntoDiagnostic;
use std::{collections::BTreeSet, path::PathBuf};
use tokio::io::AsyncWriteExt;

use nur_lib::commands;

/// A robust task runner.
#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Names of the tasks to run.
    #[clap(exclusive = true)]
    task_names: Vec<String>,

    /// Create a fresh Nurfile.
    #[clap(long, conflicts_with_all = &["list", "check", "task-names"])]
    init: bool,

    /// List all the tasks available in the Nurfile.
    #[clap(long, short, conflicts_with_all = &["init", "task-names", "check", "dry-run"])]
    list: bool,

    /// Syntax check the Nurfile and its shell commands.
    #[clap(long, conflicts_with_all = &["init", "task-names", "list", "dry-run"])]
    check: bool,

    /// Should what would be executed but don’t actually run the commands.
    #[clap(long, conflicts_with_all = &["list", "check"])]
    dry_run: bool,

    /// Specify which Nurfile to use.
    #[clap(long)]
    file: Option<PathBuf>,
}

fn build_command(cli: Cli) -> Box<dyn commands::Command> {
    if cli.init {
        return Box::new(commands::Init {
            nur_file: cli.file,
            dry_run: cli.dry_run,
        });
    }

    if cli.check {
        return Box::new(commands::Check { nur_file: cli.file });
    }

    if cli.list {
        return Box::new(commands::List { nur_file: cli.file });
    }

    if cli.dry_run {
        panic!("dry-run not yet supported");
    }

    Box::new(commands::Task {
        dry_run: cli.dry_run,
        nur_file: cli.file,
        task_names: BTreeSet::from_iter(cli.task_names),
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> miette::Result<()> {
    let cwd = std::env::current_dir().into_diagnostic()?;
    let cli = Cli::parse();
    let command = build_command(cli);

    let (tx_std, mut rx_std) = tokio::sync::mpsc::channel::<String>(1000);
    let (tx_err, mut rx_err) = tokio::sync::mpsc::channel::<String>(1000);

    let stdout_writer = async move {
        let mut stdout = tokio::io::stdout();
        while let Some(mut line) = rx_std.recv().await {
            line.push('\n');
            if stdout.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    };

    let stderr_writer = async move {
        let mut stderr = tokio::io::stderr();
        while let Some(mut line) = rx_err.recv().await {
            line.push('\n');
            if stderr.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    };

    let ctx = nur_lib::commands::Context {
        cwd,
        stdout: tx_std,
        stderr: tx_err,
    };

    let (result, (), ()) = tokio::join!(command.run(ctx), stdout_writer, stderr_writer);
    result
}

#[cfg(feature = "nu")]
fn nu() {
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
}
