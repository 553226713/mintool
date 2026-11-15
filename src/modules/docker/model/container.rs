#[derive(Clone, Debug)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,
}
impl DockerContainer {
    pub fn running(&self) -> bool {
        self.state == "running"
    }
    pub fn paused(&self) -> bool {
        self.state == "paused"
    }
    pub fn active(&self) -> bool {
        matches!(self.state.as_str(), "running" | "paused" | "restarting")
    }
    pub fn state_label(&self) -> &str {
        match self.state.as_str() {
            "running" => "运行中",
            "paused" => "已暂停",
            "restarting" => "重启中",
            "created" => "已创建",
            "exited" => "已退出",
            "dead" => "异常",
            _ => &self.state,
        }
    }
}
