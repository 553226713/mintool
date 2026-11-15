#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DockerResource {
    #[default]
    Containers,
    Images,
    Volumes,
    Networks,
    Compose,
}
impl DockerResource {
    pub const ALL: [Self; 5] = [
        Self::Containers,
        Self::Images,
        Self::Volumes,
        Self::Networks,
        Self::Compose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Containers => "容器",
            Self::Images => "镜像",
            Self::Volumes => "卷",
            Self::Networks => "网络",
            Self::Compose => "Compose",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Containers => "查看状态、日志和终端，并启停或删除容器",
            Self::Images => "管理本地镜像并复制镜像标识",
            Self::Volumes => "管理持久化数据卷",
            Self::Networks => "管理 Docker 网络",
            Self::Compose => "按项目分组查看服务，并执行启动、停止、重启和日志",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Containers => "docker",
            Self::Images => "file-archive",
            Self::Volumes => "file-database",
            Self::Networks => "columns-2",
            Self::Compose => "files",
        }
    }
}
