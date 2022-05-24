use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use goldenfile::Mint;
use miette::IntoDiagnostic;
use tokio::io::AsyncWrite;

use nur_lib::{commands::Command, nurfile::OutputOptions};

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

        let mut output_buf = Vec::new();
        let mut error_buf = Vec::new();

        let result = run_config(&parent_dir, &path, &mut output_buf, &mut error_buf).await;

        let golden_path = {
            // https://www.youtube.com/watch?v=jTqwe57ObFo
            let mut name = PathBuf::from(entry.file_name());
            name.set_extension("txt");
            name
        };

        let golden = mint.new_goldenfile(&golden_path).into_diagnostic()?;
        write_outputs(golden, &output_buf, &error_buf, result).into_diagnostic()?;
    }

    Ok(())
}

fn run_config<'a>(
    parent_dir: &Path,
    nurfile_path: &Path,
    stdout: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
    stderr: &'a mut (dyn AsyncWrite + Send + Sync + Unpin),
) -> impl std::future::Future<Output = miette::Result<()>> + 'a {
    let ctx = nur_lib::commands::Context {
        cwd: parent_dir.to_owned(),
        stdout,
        stderr,
    };

    let task_command = nur_lib::commands::Task {
        dry_run: false,
        nur_file: Some(nurfile_path.to_owned()),
        task_names: Default::default(),
        output_override: Some(OutputOptions {
            prefix: nur_lib::nurfile::PrefixStyle::Aligned,
            style: nur_lib::nurfile::OutputStyle::Grouped {
                separator: ": ".to_string(),
                separator_start: None,
                separator_end: None,
                deterministic: true,
            },
        }),
    };

    async move { task_command.run(ctx).await }
}

fn write_outputs(
    mut golden: std::fs::File,
    stdout: &[u8],
    stderr: &[u8],
    result: miette::Result<()>,
) -> std::io::Result<()> {
    golden.write_all(b"--- stdout ---\n")?;
    golden.write_all(stdout)?;

    golden.write_all(b"--- stderr ---\n")?;
    golden.write_all(stderr)?;

    if let Err(e) = result {
        let str = format!("{:?}", e);

        golden.write_all(b"--- error ---\n")?;
        golden.write_all(str.as_bytes())?;
        golden.write_all(b"\n")?;
    }

    golden.flush()?;

    Ok(())
}
