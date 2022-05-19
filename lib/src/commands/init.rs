pub struct Init {}

#[async_trait::async_trait]
impl crate::commands::Command for Init {
    async fn run(
        &self,
        ctx: crate::commands::Context,
        config: crate::nurfile::NurFile,
    ) -> miette::Result<()> {
        todo!()
    }
}
