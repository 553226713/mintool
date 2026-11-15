use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct LocalEntry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub size: u64,
}
