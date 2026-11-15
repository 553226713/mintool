#[derive(Clone, Debug)]
pub struct DockerImage {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
}
impl DockerImage {
    pub fn reference(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }
}
