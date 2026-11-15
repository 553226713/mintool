//! Docker removal confirmations.

use crate::app::MintoolApp;
use crate::ui::dialogs::DialogContent;
use gpui::*;

#[derive(Clone)]
pub enum DockerDialog {
    Container(String, String),
    Image(String, String),
    Volume(String, String),
    Network(String, String),
}

impl DialogContent for DockerDialog {
    fn title(&self) -> String {
        match self {
            Self::Container(..) => "删除容器",
            Self::Image(..) => "删除镜像",
            Self::Volume(..) => "删除数据卷",
            Self::Network(..) => "删除网络",
        }
        .into()
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn subject(&self) -> Option<String> {
        match self {
            Self::Container(_, name)
            | Self::Image(_, name)
            | Self::Volume(_, name)
            | Self::Network(_, name) => Some(name.clone()),
        }
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        Some(
            match self {
                Self::Container(..) => "强制删除此容器，容器内未保存的数据会丢失。",
                Self::Image(..) => "删除此本地镜像。正在使用该镜像的容器不会被删除。",
                Self::Volume(..) => "永久删除此数据卷及其中的数据，无法恢复。",
                Self::Network(..) => "删除此 Docker 网络。正在连接该网络的容器会导致删除失败。",
            }
            .to_string(),
        )
    }
    fn confirm_label(&self) -> &'static str {
        "删除"
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        _value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Container(id, _) => app.remove_docker_container(id.clone(), cx),
            Self::Image(id, _) => app.remove_docker_image(id.clone(), cx),
            Self::Volume(name, _) => app.remove_docker_volume(name.clone(), cx),
            Self::Network(id, _) => app.remove_docker_network(id.clone(), cx),
        }
        Ok(())
    }
}
