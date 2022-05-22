use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use goldenfile::Mint;
use tokio::sync::mpsc::{self, Sender};

use nur_lib::{commands::Command, commands::Message};

#[tokio::test]
async fn run_golden_tests() -> std::io::Result<()> {
    std::env::set_var("NO_COLOR", "1");

    let parent_dir: PathBuf = "tests/goldenfiles".into();
    let mut mint = Mint::new(&parent_dir);

    let yaml_extension: std::ffi::OsString = "yml".into();
    for entry in fs::read_dir(&parent_dir)? {
        let entry = entry?;
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

        let mut golden = mint.new_goldenfile(&golden_path)?;

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
