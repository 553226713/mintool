use std::path::PathBuf;

#[derive(Clone, Copy)]
pub enum DirectoryTarget {
    DialogInput,
    DatabaseFormPath,
}

pub struct DirectoryResult {
    pub path: PathBuf,
    pub generation: u64,
    pub target: DirectoryTarget,
}
