use miette::IntoDiagnostic;
use nu_table::StyledString;

use crate::StatusMessage;

pub struct List {
    pub nur_file: Option<std::path::PathBuf>,
}

#[async_trait::async_trait]
impl crate::commands::Command for List {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (_, config) = crate::nurfile::load_config(&ctx.cwd, &self.nur_file)?;

        let taskdata: Vec<Vec<StyledString>> = config
            .tasks // already sorted by virtue of being in a BTreeMap
            .into_iter()
            .map(|(name, task)| {
                vec![
                    StyledString::new(name, Default::default()),
                    StyledString::new(task.description, Default::default()),
                ]
            })
            .collect();

        let table = nu_table::Table::new(
            vec![
                StyledString::new("Name".to_string(), Default::default()),
                StyledString::new("Description".to_string(), Default::default()),
            ],
            taskdata,
            nu_table::Theme::rounded(),
        );

        let config = Default::default();
        let color_hm = Default::default();
        let table = nu_table::draw_table(&table, 80, &color_hm, &config);

        ctx.tx
            .send(StatusMessage::StdOut(table))
            .await
            .into_diagnostic()
    }
}
