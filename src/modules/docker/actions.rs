use super::{
    cli,
    dialogs::DockerDialog,
    model,
    model::{DockerResource, DockerView},
};
use crate::app::MintoolApp;
use crate::modules::{ModuleId, SessionKey};
use crate::ui::dialogs::Dialog;
use gpui::{Context, Window};
use std::time::{Duration, Instant};

impl MintoolApp {
    pub(crate) fn select_docker_container(&mut self, id: String, cx: &mut Context<Self>) {
        self.modules.docker.selected = Some(id);
        self.modules.active = ModuleId::Docker;
        self.settings.active_module = ModuleId::Docker;
        self.dirty = Some(Instant::now());
        cx.notify();
    }

    /// Show logs/exec for a container in the right-hand auxiliary panel while
    /// keeping the resource list visible in the main area.
    pub(crate) fn open_docker_session(
        &mut self,
        id: String,
        view: DockerView,
        cx: &mut Context<Self>,
    ) {
        if self
            .modules
            .docker
            .opened
            .as_ref()
            .is_some_and(|(opened, opened_view)| opened == &id && *opened_view == view)
        {
            self.focus_pending = true;
            self.changed(cx);
            return;
        }
        if let Some((previous_id, previous_view)) = self.modules.docker.opened.take() {
            self.remove_session(
                &SessionKey::docker(model::session_id(&previous_id, previous_view)),
                cx,
            );
        }
        let key = SessionKey::docker(model::session_id(&id, view));
        self.modules.docker.selected = Some(id.clone());
        self.modules.docker.opened = Some((id, view));
        self.modules.docker.show_resources = true;
        self.modules.active = ModuleId::Docker;
        self.settings.active_module = ModuleId::Docker;
        self.ensure_session(&key);
        self.ensure_docker_loaded(cx);
        self.focus_pending = true;
        self.dirty = Some(Instant::now());
        self.changed(cx);
    }

    pub(crate) fn show_docker_stats(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match cli::stats_summary() {
            Ok(text) => self.show_dialog(Dialog::Docker(DockerDialog::Stats { text }), window, cx),
            Err(error) => {
                self.error = Some(format!("读取资源统计失败：{error:#}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn show_container_details(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self
            .modules
            .docker
            .container(&id)
            .map(|container| container.name.clone())
            .unwrap_or_else(|| id.clone());
        match cli::inspect_container(&id) {
            Ok(text) => self.show_dialog(
                Dialog::Docker(DockerDialog::Details { name, text }),
                window,
                cx,
            ),
            Err(error) => {
                self.error = Some(format!("读取容器详情失败：{error:#}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn prune_docker_resource(
        &mut self,
        resource: DockerResource,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { cli::prune(resource) }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        this.error = Some(format!("已清理未使用的{}", resource.label()));
                        this.refresh_docker(cx);
                    }
                    Err(error) => {
                        this.error = Some(format!("清理失败：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn create_docker_volume(&mut self, name: String, cx: &mut Context<Self>) {
        if name.trim().is_empty() {
            self.error = Some("请填写数据卷名称".into());
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { cli::create_volume(name.trim()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some("已创建数据卷".into());
                        this.refresh_docker(cx);
                    }
                    Err(error) => this.error = Some(format!("创建数据卷失败：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn create_docker_network(&mut self, name: String, cx: &mut Context<Self>) {
        if name.trim().is_empty() {
            self.error = Some("请填写网络名称".into());
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { cli::create_network(name.trim()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some("已创建网络".into());
                        this.refresh_docker(cx);
                    }
                    Err(error) => this.error = Some(format!("创建网络失败：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn pull_docker_image(&mut self, reference: String, cx: &mut Context<Self>) {
        if reference.trim().is_empty() {
            self.error = Some("请填写镜像名称".into());
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let requested = reference.clone();
            let result = executor
                .spawn(async move { cli::pull_image(&reference) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some(format!("已拉取 {requested}"));
                        this.refresh_docker(cx);
                    }
                    Err(error) => this.error = Some(format!("拉取失败：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn tag_docker_image(
        &mut self,
        id: String,
        reference: String,
        cx: &mut Context<Self>,
    ) {
        if reference.trim().is_empty() {
            self.error = Some("请填写镜像标签".into());
            cx.notify();
            return;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let requested = reference.clone();
            let result = executor
                .spawn(async move { cli::tag_image(&id, &reference) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some(format!("已标记为 {requested}"));
                        this.refresh_docker(cx);
                    }
                    Err(error) => this.error = Some(format!("标记失败：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn run_docker_image(
        &mut self,
        reference: String,
        name: String,
        ports: String,
        env: String,
        mounts: String,
        command: String,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let requested = reference.clone();
            let result =
                executor
                    .spawn(async move {
                        cli::run_image(&reference, &name, &ports, &env, &mounts, &command)
                    })
                    .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(id) => {
                        let short = id.get(..12).unwrap_or(&id).to_string();
                        this.error = Some(format!("已从 {requested} 启动容器 {short}"));
                        this.refresh_docker(cx);
                    }
                    Err(error) => {
                        this.error = Some(format!("启动容器失败：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Reload the log panel after changing tail/timestamp options.
    pub(crate) fn restart_docker_log(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .modules
            .docker
            .opened
            .as_ref()
            .is_some_and(|(opened, view)| opened == &id && *view == DockerView::Logs)
        {
            self.close_docker_session(cx);
            self.open_docker_session(id, DockerView::Logs, cx);
        }
    }

    /// Close the auxiliary logs/exec panel and stop its process.
    pub(crate) fn close_docker_session(&mut self, cx: &mut Context<Self>) {
        if let Some((id, view)) = self.modules.docker.opened.take() {
            self.remove_session(&SessionKey::docker(model::session_id(&id, view)), cx);
        }
        self.modules.docker.show_resources = true;
        self.focus_pending = true;
        self.changed(cx);
    }

    pub(crate) fn select_docker_resource(
        &mut self,
        resource: DockerResource,
        cx: &mut Context<Self>,
    ) {
        self.modules.docker.resource = resource;
        self.modules.docker.show_resources = true;
        self.modules.docker.selected = None;
        self.modules.active = ModuleId::Docker;
        self.settings.active_module = ModuleId::Docker;
        self.ensure_docker_loaded(cx);
        if resource == DockerResource::Compose {
            self.ensure_compose_loaded(cx);
        }
        self.dirty = Some(Instant::now());
        cx.notify();
    }

    pub(crate) fn ensure_docker_loaded(&mut self, cx: &mut Context<Self>) {
        if self.modules.docker.last_refresh.is_none() && !self.modules.docker.loading {
            self.refresh_docker(cx);
        }
    }

    /// Refresh the Docker snapshot while the module is visible and the data is
    /// older than the refresh interval. Called from the UI tick.
    pub(crate) fn auto_refresh_docker(&mut self, cx: &mut Context<Self>) {
        if self.modules.active != ModuleId::Docker
            || self.modules.docker.loading
            || self.modules.docker.resource == DockerResource::Compose
        {
            return;
        }
        let interval = Duration::from_secs(self.settings.docker_refresh_seconds.clamp(2, 60));
        if self
            .modules
            .docker
            .last_refresh
            .is_none_or(|at| at.elapsed() >= interval)
        {
            self.refresh_docker(cx);
        }
    }

    pub(crate) fn refresh_docker(&mut self, cx: &mut Context<Self>) {
        if self.modules.docker.loading {
            return;
        }
        self.modules.docker.loading = true;
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { cli::snapshot() }).await;
            let _ = this.update(cx, |this, cx| {
                this.modules.docker.loading = false;
                this.modules.docker.last_refresh = Some(Instant::now());
                match result {
                    Ok((containers, images, volumes, networks)) => {
                        if let Some(selected) = this.modules.docker.selected.clone() {
                            if !selected.starts_with("compose-")
                                && !containers.iter().any(|container| container.id == selected)
                            {
                                this.modules.docker.selected = None;
                            }
                        }
                        if let Some((opened, view)) = this.modules.docker.opened.clone() {
                            if !opened.starts_with("compose-")
                                && !containers.iter().any(|container| container.id == opened)
                            {
                                this.modules.docker.opened = None;
                                this.modules.docker.show_resources = true;
                                this.remove_session(
                                    &SessionKey::docker(model::session_id(&opened, view)),
                                    cx,
                                );
                            }
                        }
                        this.modules.docker.containers = containers;
                        this.modules.docker.images = images;
                        this.modules.docker.volumes = volumes;
                        this.modules.docker.networks = networks;
                        this.modules.docker.error = None;
                    }
                    Err(error) => this.modules.docker.error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn ensure_compose_loaded(&mut self, cx: &mut Context<Self>) {
        if self.modules.docker.compose_loaded || self.modules.docker.compose_loading {
            return;
        }
        self.load_compose(cx);
    }

    pub(crate) fn load_compose(&mut self, cx: &mut Context<Self>) {
        if self.modules.docker.compose_loading {
            return;
        }
        self.modules.docker.compose_loading = true;
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { cli::compose_projects() }).await;
            let _ = this.update(cx, |this, cx| {
                this.modules.docker.compose_loading = false;
                this.modules.docker.compose_loaded = true;
                match result {
                    Ok(projects) => {
                        this.modules.docker.compose = projects;
                        this.modules.docker.error = None;
                    }
                    Err(error) => {
                        this.modules.docker.error =
                            Some(format!("读取 Compose 项目失败：{error:#}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn compose_action(
        &mut self,
        project: String,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { cli::compose_action(&project, action) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some(format!(
                            "Compose 操作已完成：{}",
                            match action {
                                "up" => "启动",
                                "down" => "停止",
                                _ => "重启",
                            }
                        ));
                        this.load_compose(cx);
                    }
                    Err(error) => this.error = Some(format!("Compose 操作失败：{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_compose_logs(&mut self, project: String, cx: &mut Context<Self>) {
        self.open_docker_session(format!("compose-{project}"), DockerView::Logs, cx);
    }

    pub(crate) fn docker_action(
        &mut self,
        id: String,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { cli::action(&id, action) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.modules.docker.error = None;
                        this.refresh_docker(cx);
                    }
                    Err(error) => {
                        this.modules.docker.error = Some(format!("{error:#}"));
                        this.modules.docker.last_refresh = Some(Instant::now());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_docker_container(&mut self, id: String, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        let container_id = id.clone();
        let task_id = id.clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { cli::remove_container(&task_id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        if let Some((opened, view)) = this.modules.docker.opened.clone() {
                            if opened == container_id {
                                this.modules.docker.opened = None;
                                this.remove_session(
                                    &SessionKey::docker(model::session_id(&container_id, view)),
                                    cx,
                                );
                            }
                        }
                        this.modules.docker.error = None;
                        this.refresh_docker(cx);
                    }
                    Err(error) => {
                        this.modules.docker.error = Some(format!("{error:#}"));
                        this.modules.docker.last_refresh = Some(Instant::now());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_docker_image(&mut self, id: String, cx: &mut Context<Self>) {
        self.remove_docker_resource(
            id,
            cli::remove_image,
            |module, id| module.images.retain(|image| image.id != id),
            cx,
        );
    }

    pub(crate) fn remove_docker_volume(&mut self, name: String, cx: &mut Context<Self>) {
        self.remove_docker_resource(
            name,
            cli::remove_volume,
            |module, name| module.volumes.retain(|volume| volume.name != name),
            cx,
        );
    }

    pub(crate) fn remove_docker_network(&mut self, id: String, cx: &mut Context<Self>) {
        self.remove_docker_resource(
            id,
            cli::remove_network,
            |module, id| module.networks.retain(|network| network.id != id),
            cx,
        );
    }

    fn remove_docker_resource(
        &mut self,
        id: String,
        remove: fn(&str) -> anyhow::Result<()>,
        remove_cached: fn(&mut super::DockerModule, &str),
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        let task_id = id.clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { remove(&task_id) }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        remove_cached(&mut this.modules.docker, &id);
                        if this.modules.docker.selected.as_deref() == Some(id.as_str()) {
                            this.modules.docker.selected = None;
                        }
                        this.modules.docker.error = None;
                        this.modules.docker.last_refresh = Some(Instant::now());
                    }
                    Err(error) => {
                        this.modules.docker.error = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
