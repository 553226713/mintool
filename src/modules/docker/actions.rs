use super::{
    cli, model,
    model::{DockerResource, DockerView},
};
use crate::app::MintoolApp;
use crate::modules::{ModuleId, SessionKey};
use gpui::Context;
use std::time::Instant;

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
        self.dirty = Some(Instant::now());
        cx.notify();
    }

    pub(crate) fn ensure_docker_loaded(&mut self, cx: &mut Context<Self>) {
        if self.modules.docker.last_refresh.is_none() && !self.modules.docker.loading {
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
                            if !containers.iter().any(|container| container.id == selected) {
                                this.modules.docker.selected = None;
                            }
                        }
                        if let Some((opened, view)) = this.modules.docker.opened.clone() {
                            if !containers.iter().any(|container| container.id == opened) {
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
