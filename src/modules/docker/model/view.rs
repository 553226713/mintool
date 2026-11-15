#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockerView {
    Logs,
    Exec,
}
impl DockerView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Logs => "日志",
            Self::Exec => "终端",
        }
    }
    pub(super) fn suffix(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::Exec => "exec",
        }
    }
}
