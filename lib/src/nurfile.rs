use std::collections::BTreeMap;

#[derive(Debug)]
pub struct NurFile {
    pub version: crate::version::Version,

    pub lets: Vec<Let>,

    pub tasks: BTreeMap<String, NurTask>,

    pub env: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct Let {}

#[derive(Debug, Clone)]
pub struct NurTask {
    pub env: BTreeMap<String, String>,
    pub description: String,
    pub dependencies: Vec<String>,
    pub commands: Vec<NurCommand>,
}

#[derive(Debug, Clone)]
pub struct NurCommand {
    pub env: BTreeMap<String, String>,
    pub sh: String,
    pub ignore_result: bool,
}
