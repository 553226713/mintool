use super::model::DockerResource;
use super::DockerModule;
use crate::ui::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

pub(super) fn docker_table_header(columns: &[&str]) -> AnyElement {
    let mut row = div()
        .mt_4()
        .mb_1()
        .h(px(28.))
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(theme::border())
        .text_size(px(9.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::muted());
    for (index, column) in columns.iter().enumerate() {
        row = row.child(
            div()
                .when(index == 0, |cell| cell.flex_1())
                .when(index > 0 && index + 1 < columns.len(), |cell| {
                    cell.w(px(if index == 1 { 120. } else { 150. }))
                })
                .when(index + 1 == columns.len(), |cell| cell.w(px(230.)))
                .child((*column).to_string()),
        );
    }
    row.into_any_element()
}

pub(super) fn docker_resource_total(module: &DockerModule, resource: DockerResource) -> usize {
    match resource {
        DockerResource::Containers => module.containers.len(),
        DockerResource::Images => module.images.len(),
        DockerResource::Volumes => module.volumes.len(),
        DockerResource::Networks => module.networks.len(),
    }
}

pub(super) fn filtered_docker_resource_count(
    module: &DockerModule,
    resource: DockerResource,
    filter: &str,
) -> usize {
    match resource {
        DockerResource::Containers => module
            .containers
            .iter()
            .filter(|container| {
                filter.is_empty()
                    || container.name.to_lowercase().contains(filter)
                    || container.image.to_lowercase().contains(filter)
                    || container.id.to_lowercase().contains(filter)
            })
            .count(),
        DockerResource::Images => module
            .images
            .iter()
            .filter(|image| {
                filter.is_empty()
                    || image.reference().to_lowercase().contains(filter)
                    || image.id.to_lowercase().contains(filter)
            })
            .count(),
        DockerResource::Volumes => module
            .volumes
            .iter()
            .filter(|volume| {
                filter.is_empty()
                    || volume.name.to_lowercase().contains(filter)
                    || volume.driver.to_lowercase().contains(filter)
                    || volume.mountpoint.to_lowercase().contains(filter)
            })
            .count(),
        DockerResource::Networks => module
            .networks
            .iter()
            .filter(|network| {
                filter.is_empty()
                    || network.name.to_lowercase().contains(filter)
                    || network.driver.to_lowercase().contains(filter)
                    || network.id.to_lowercase().contains(filter)
            })
            .count(),
    }
}
