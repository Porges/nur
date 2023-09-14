use std::{io::Write, path::Path};

use miette::Result;

use nur_lib::{commands::Command, nurfile::OutputOptions};

#[test]
fn check() -> Result<()> {
    // https://www.youtube.com/watch?v=jTqwe57ObFo

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

    // normalize paths to avoid spurious changes
    insta::with_settings!({filters => vec![("[^\"\\[]+\\.yml", "[…].yml")]}, {
        insta::glob!("test_inputs/*.yml", |path| {
            let mut output_buf = Vec::new();
            let mut error_buf = Vec::new();

            let result = run_config(path, &mut output_buf, &mut error_buf);
            let golden = prep_output(&output_buf, &error_buf, result);
            insta::assert_snapshot!(golden);
        });
    });

    Ok(())
}

fn run_config(
    nurfile_path: &Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> miette::Result<()> {
    let parent_dir = nurfile_path.parent().unwrap();

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
                separator: "│".to_string(),
                separator_first: Some("╭".to_string()),
                separator_last: Some("╰".to_string()),
                deterministic: true,
                only_on_failure: false,
            },
        }),
    };

    task_command.run(ctx)
}

fn prep_output(stdout: &[u8], stderr: &[u8], result: Result<()>) -> String {
    let error = match result {
        Err(e) => Some(format!("{:?}", e)),
        Ok(()) => None,
    };

    let result = Golden {
        stdout: from_str(stdout),
        stderr: from_str(stderr),
        error,
    };

    serde_yaml::to_string(&result).unwrap()
}

fn from_str(x: &[u8]) -> Option<String> {
    match String::from_utf8_lossy(x) {
        x if x.is_empty() => None,
        x => Some(x.to_string()),
    }
}

#[serde_with::skip_serializing_none]
#[derive(serde::Serialize)]
struct Golden {
    stdout: Option<String>,
    stderr: Option<String>,
    error: Option<String>,
}
