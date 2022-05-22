#[derive(knuffel::Decode, Debug)]
pub struct NurKdl {
    pub version: String,

    #[knuffel(children(name = "let"))]
    pub lets: Vec<Let>,

    #[knuffel(children(name = "task"))]
    pub tasks: Vec<Task>,
}

impl From<NurKdl> for crate::nurfile::NurFile {
    fn from(_: NurKdl) -> Self {
        todo!()
    }
}

#[derive(knuffel::Decode, Debug)]
pub struct Let {
    #[knuffel(argument)]
    pub name: String,
    #[knuffel(argument)]
    pub value: String,

    #[knuffel(property)]
    pub env: Option<String>,
}

#[derive(knuffel::Decode, Debug)]
pub struct Task {
    #[knuffel(argument)]
    pub name: String,

    #[knuffel(children(name = "exec"))]
    pub execs: Vec<Exec>,
}

#[derive(knuffel::Decode, Debug)]
pub struct Exec {
    #[knuffel(argument)]
    pub input: String,
}

pub fn parse(path: &std::path::Path, input: &str) -> miette::Result<crate::nurfile::NurFile> {
    let displayable_filename = path.display().to_string();
    let nf: NurKdl = knuffel::parse(&displayable_filename, input)?;
    Ok(nf.into())
}
