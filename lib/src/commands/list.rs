use miette::IntoDiagnostic;
use owo_colors::OwoColorize;

pub struct List {
    pub nur_file: Option<std::path::PathBuf>,
}

impl crate::commands::Command for List {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let wrap_opts = textwrap::Options::with_termwidth()
            .initial_indent("  - ")
            .subsequent_indent("    ");

        let (_, config) = crate::nurfile::load_config(&ctx.cwd, self.nur_file.as_deref())?;

        let name_style = owo_colors::Style::new().bold();

        // tasks are already sorted by name by virtue of being in a BTreeMap
        for (name, task) in config.tasks {
            writeln!(ctx.stdout, "{}", name.style(name_style)).into_diagnostic()?;
            for line in textwrap::wrap(&task.description, &wrap_opts) {
                writeln!(ctx.stdout, "{line}").into_diagnostic()?;
            }
        }

        Ok(())
    }
}
