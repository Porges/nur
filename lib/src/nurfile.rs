use std::collections::BTreeMap;

#[derive(Debug)]
pub struct NurFile {
    pub version: crate::version::Version,

    pub lets: Vec<Let>,

    pub tasks: BTreeMap<String, Task>,
}

#[derive(Debug)]
pub struct Let {}

#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
    pub dependencies: Vec<String>,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub sh: String,
    pub ignore_result: bool,
}
