use std::path::PathBuf;

pub struct DirectoryResult {
    pub path: PathBuf,
    pub generation: u64,
}
