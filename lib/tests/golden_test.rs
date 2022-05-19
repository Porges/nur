use goldenfile::Mint;
use nur_lib::commands::Command;
use std::{fs, io::Write, path::PathBuf};
use tokio::sync::mpsc;

#[tokio::test]
async fn run_golden_tests() -> std::io::Result<()> {
    let parent_dir: PathBuf = "tests/goldenfiles".into();
    let mut mint = Mint::new(&parent_dir);

    let yaml_extension: std::ffi::OsString = "yml".into();
    for entry in fs::read_dir(&parent_dir)? {
        let entry = entry?;
        if entry.path().extension() != Some(&yaml_extension) {
            continue;
        }

        let (tx_std, mut rx_std) = mpsc::channel(10);
        let (tx_err, mut rx_err) = mpsc::channel(10);

        let read_std = tokio::spawn(async move {
            let mut output = Vec::new();
            while let Some(line) = rx_std.recv().await {
                output.push(line);
            }
            output
        });

        let read_err = tokio::spawn(async move {
            let mut output = Vec::new();
            while let Some(line) = rx_err.recv().await {
                output.push(line);
            }
            output
        });

        let ctx = nur_lib::commands::Context {
            cwd: parent_dir.clone(),
            stdout: tx_std,
            stderr: tx_err,
        };

        let nurfile_path = entry.path();
        let contents = fs::read_to_string(&nurfile_path)?;
        let config =
            nur_lib::nurfile_yaml::parse(&nurfile_path, &contents).expect("couldn't parse file");

        let task_command = nur_lib::commands::Task {
            task_names: Default::default(),
        };

        task_command.run(ctx, config).await.expect("not to fail");

        let golden_path = {
            // https://www.youtube.com/watch?v=jTqwe57ObFo
            let mut name = PathBuf::from(entry.path().file_name().expect("filename"));
            name.set_extension("txt");
            name
        };

        let mut golden = mint.new_goldenfile(&golden_path)?;

        golden.write_all(b"STDOUT:\n")?;
        for line in read_std.await? {
            golden.write_all(line.as_bytes())?;
            golden.write_all(b"\n")?;
        }

        golden.write_all(b"---\nSTDERR:\n")?;
        for line in read_err.await? {
            golden.write_all(line.as_bytes())?;
            golden.write_all(b"\n")?;
        }

        golden.flush()?;
    }

    Ok(())
}
