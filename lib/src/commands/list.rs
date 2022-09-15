use miette::IntoDiagnostic;
use nu_table::StyledString;
use terminal_size::{terminal_size, Height, Width};
use tokio::io::AsyncWriteExt;

pub struct List {
    pub nur_file: Option<std::path::PathBuf>,
}

#[async_trait::async_trait]
impl crate::commands::Command for List {
    async fn run<'c>(&self, ctx: crate::commands::Context<'c>) -> miette::Result<()> {
        let (_, config) = crate::nurfile::load_config(&ctx.cwd, &self.nur_file)?;

        let text_style = Default::default();
        let taskdata: Vec<Vec<StyledString>> = config
            .tasks // already sorted by virtue of being in a BTreeMap
            .into_iter()
            .map(|(name, task)| {
                vec![
                    StyledString::new(name, text_style),
                    StyledString::new(task.description, text_style),
                ]
            })
            .collect();

        let table = nu_table::Table::new(
            vec![
                StyledString::new("Name".to_string(), text_style),
                StyledString::new("Description".to_string(), text_style),
            ],
            taskdata,
            nu_table::TableTheme::rounded(),
        );

        let (Width(term_width), _) = terminal_size().unwrap_or((Width(80), Height(24)));

        let config = Default::default();
        let color_hm = Default::default();
        let alignments = Default::default();
        if let Some(mut table) = table.draw_table(&config, &color_hm, alignments, term_width as usize) {
            table.push('\n');

            ctx.stdout
                .write_all(table.as_bytes())
                .await
                .into_diagnostic()?;
        } else {
            ctx.stderr
                .write_all(b"Unable to fit table to terminal width")
                .await
                .into_diagnostic()?;
        }

        Ok(())
    }
}
