use miette::IntoDiagnostic;
use nu_ansi_term::Style;
use nu_table::{NuTable, NuTableConfig, TableTheme, TextStyle};
use tabled::grid::records::vec_records::CellInfo;
use terminal_size::{terminal_size, Height, Width};

pub struct List {
    pub nur_file: Option<std::path::PathBuf>,
}

impl crate::commands::Command for List {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (_, config) = crate::nurfile::load_config(&ctx.cwd, self.nur_file.as_deref())?;

        let mut taskdata: Vec<Vec<CellInfo<String>>> = config
            .tasks // already sorted by virtue of being in a BTreeMap
            .into_iter()
            .map(|(name, task)| vec![CellInfo::new(name), CellInfo::new(task.description)])
            .collect();

        taskdata.insert(
            0,
            vec![
                CellInfo::new("Name".to_string()),
                CellInfo::new("Description".to_string()),
            ],
        );

        let mut table = NuTable::from(taskdata);
        table.set_data_style(TextStyle::basic_left());
        table.set_header_style(TextStyle::basic_left().style(Style::new().bold()));

        let config = NuTableConfig {
            theme: TableTheme::rounded(),
            with_header: true,
            ..Default::default()
        };

        let (Width(term_width), _) = terminal_size().unwrap_or((Width(80), Height(24)));

        if let Some(mut table) = table.draw(config, term_width as usize) {
            table.push('\n');

            ctx.stdout.write_all(table.as_bytes()).into_diagnostic()?;
        } else {
            ctx.stderr
                .write_all(b"Unable to fit table to terminal width")
                .into_diagnostic()?;
        }

        Ok(())
    }
}
