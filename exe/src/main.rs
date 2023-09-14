use clap::Parser;
use miette::IntoDiagnostic;
use std::{collections::BTreeSet, path::PathBuf};

use nur_lib::{
    commands,
    nurfile::{OutputOptions, OutputStyle, PrefixStyle},
};

/// A robust task runner.
#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Names of the tasks to run.
    #[clap(exclusive = true)]
    task_names: Vec<String>,

    /// Create a fresh Nurfile.
    #[clap(long, conflicts_with_all = &["list", "check", "task_names"])]
    init: bool,

    /// List all the tasks available in the Nurfile.
    #[clap(long, short, conflicts_with_all = &["init", "task_names", "check", "dry_run"])]
    list: bool,

    /// Syntax check the Nurfile and its shell commands.
    #[clap(long, conflicts_with_all = &["init", "task_names", "list", "dry_run"])]
    check: bool,

    /// Should what would be executed but don’t actually run the commands.
    #[clap(long, conflicts_with_all = &["list", "check"])]
    dry_run: bool,

    /// Specify which Nurfile to use.
    #[clap(long)]
    file: Option<PathBuf>,
}

// the 'subcommands' are:
// * <none>: run the list of tasks
// * --init: create a sample config file
// * --list: list all tasks
// * --check: syntax-check the config file

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

    // if we are running in a Github action, automatically use a nice format
    let output_override = if std::env::var_os("GITHUB_ACTIONS") == Some("true".into()) {
        Some(OutputOptions {
            prefix: PrefixStyle::NoPrefix,
            style: OutputStyle::Grouped {
                separator: String::new(),
                separator_first: Some("::group::".to_string()),
                separator_last: Some("::endgroup::".to_string()),
                deterministic: true,
                only_on_failure: false,
            },
        })
    } else {
        None
    };

    Box::new(commands::Task {
        dry_run: cli.dry_run,
        nur_file: cli.file,
        task_names: BTreeSet::from_iter(cli.task_names),
        output_override,
    })
}

fn main() -> miette::Result<()> {
    let cwd = std::env::current_dir().into_diagnostic()?;
    let cli = Cli::parse();
    let command = build_command(cli);

    let ctx = nur_lib::commands::Context {
        cwd,
        stdout: &mut std::io::stdout().lock(),
        stderr: &mut std::io::stderr().lock(),
    };

    command.run(ctx)
}
