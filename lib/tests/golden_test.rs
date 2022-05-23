use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use goldenfile::Mint;
use miette::IntoDiagnostic;
use tokio::sync::mpsc::{self, Sender};

use nur_lib::{commands::Command, commands::Message};

#[tokio::test]
async fn run_golden_tests() -> miette::Result<()> {
    // override default hook,  since we need the output
    // to be consistent regardless of where it is running
    miette::set_hook(Box::new(|_diag| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(false)
                .unicode(true)
                .color(false)
                .width(132)
                .build(),
        )
    }))?;

    let parent_dir: PathBuf = "tests/goldenfiles".into();
    let mut mint = Mint::new(&parent_dir);

    let yaml_extension: std::ffi::OsString = "yml".into();
    for entry in fs::read_dir(&parent_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if path.extension() != Some(&yaml_extension) {
            continue;
        }

        let (tx, mut rx) = mpsc::channel(10);

        let (result, (stdout, stderr)) =
            tokio::join!(run_config(&parent_dir, &path, tx), async move {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                while let Some(line) = rx.recv().await {
                    match line {
                        Message::Out(line) => stdout.push(line),
                        Message::Err(line) => stderr.push(line),
                    }
                }

                (stdout, stderr)
            });

        let golden_path = {
            // https://www.youtube.com/watch?v=jTqwe57ObFo
            let mut name = PathBuf::from(entry.file_name());
            name.set_extension("txt");
            name
        };

        let golden = mint.new_goldenfile(&golden_path).into_diagnostic()?;
        write_outputs(golden, stdout, stderr, result).into_diagnostic()?;
    }

    Ok(())
}

async fn run_config(
    parent_dir: &Path,
    nurfile_path: &Path,
    output: Sender<nur_lib::commands::Message>,
) -> miette::Result<()> {
    let ctx = nur_lib::commands::Context {
        cwd: parent_dir.to_owned(),
        output,
    };

    let task_command = nur_lib::commands::Task {
        dry_run: false,
        nur_file: Some(nurfile_path.to_owned()),
        task_names: Default::default(),
    };

    let result = task_command.run(ctx).await?;
    Ok(result)
}

fn write_outputs(
    mut golden: std::fs::File,
    stdout: Vec<String>,
    stderr: Vec<String>,
    result: miette::Result<()>,
) -> std::io::Result<()> {
    golden.write_all(b"--- stdout ---\n")?;
    for line in stdout {
        golden.write_all(line.as_bytes())?;
        golden.write_all(b"\n")?;
    }

    golden.write_all(b"--- stderr ---\n")?;
    for line in stderr {
        golden.write_all(line.as_bytes())?;
        golden.write_all(b"\n")?;
    }

    if let Err(e) = result {
        let str = format!("{:?}", e);

        golden.write_all(b"--- error ---\n")?;
        golden.write_all(str.as_bytes())?;
        golden.write_all(b"\n")?;
    }

    golden.flush()?;

    Ok(())
}
