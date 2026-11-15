use super::{DockerContainer, DockerView};
use crate::model::session::{
    ModuleId, SessionCapabilities, SessionKey, SessionView, TerminalLauncher,
};
use crate::modules::docker::cli;

impl SessionView {
    /// Neutral workspace description owned by the Docker module.
    pub fn docker(container: &DockerContainer, view: DockerView) -> Self {
        let args = match view {
            DockerView::Logs => cli::logs_spec(&container.id),
            DockerView::Exec => cli::exec_spec(&container.id),
        };
        Self {
            key: SessionKey::new(ModuleId::Docker, session_id(&container.id, view)),
            title: format!("{} · {}", container.name, view.label()),
            subtitle: format!("{} · {}", container.image, container.state_label()),
            cwd: crate::services::desktop::home_dir(),
            launcher: TerminalLauncher::Docker {
                program: cli::binary(),
                args,
            },
            capabilities: SessionCapabilities::DOCKER,
        }
    }
}

pub fn session_id(container_id: &str, view: DockerView) -> String {
    format!("{}-{}", container_id, view.suffix())
}

pub fn parse_session_id(id: &str) -> Option<(String, DockerView)> {
    if let Some(container) = id.strip_suffix("-logs") {
        return Some((container.to_string(), DockerView::Logs));
    }
    if let Some(container) = id.strip_suffix("-exec") {
        return Some((container.to_string(), DockerView::Exec));
    }
    None
}
