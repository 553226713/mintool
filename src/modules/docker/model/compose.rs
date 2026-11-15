#[derive(Clone, Debug, Default)]
pub struct ComposeProject {
    pub name: String,
    pub status: String,
    pub config_files: String,
    pub services: Vec<ComposeService>,
}

#[derive(Clone, Debug, Default)]
pub struct ComposeService {
    pub name: String,
    pub service: String,
    pub state: String,
    pub ports: String,
}
