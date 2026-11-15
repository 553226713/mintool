use crate::ui::format::format_size;
use crate::ui::theme;
use anyhow::{ensure, Context as _, Result};
use gpui::*;
use image::ImageReader;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ANIMATED_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_ANIMATED_IMAGE_PIXELS: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
struct ImageInfo {
    width: u32,
    height: u32,
    bytes: u64,
}

enum PreviewState {
    Loading,
    Ready(ImageInfo),
    Error(String),
}

pub struct ImagePreview {
    path: PathBuf,
    height: f32,
    state: PreviewState,
    _task: Task<()>,
}

impl ImagePreview {
    pub fn new(path: PathBuf, height: f32, cx: &mut Context<Self>) -> Self {
        let executor = cx.background_executor().clone();
        let loading_path = path.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { inspect_image(&loading_path) })
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    this.state = match result {
                        Ok(info) => PreviewState::Ready(info),
                        Err(error) => PreviewState::Error(format!("{error:#}")),
                    };
                    cx.notify();
                })
                .ok();
            }
        });
        cx.on_release(|this, cx| {
            cx.remove_asset::<ImgResourceLoader>(&this.path.clone().into());
        })
        .detach();
        Self {
            path,
            height,
            state: PreviewState::Loading,
            _task: task,
        }
    }

    pub fn resize(&mut self, height: f32, cx: &mut Context<Self>) {
        if (self.height - height).abs() > 0.5 {
            self.height = height;
            cx.notify();
        }
    }

    fn header(&self, info: Option<&ImageInfo>) -> AnyElement {
        let name = self
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let details = info.map_or_else(
            || "正在读取图片…".to_string(),
            |info| {
                format!(
                    "{}×{} · {} · 自适应窗口",
                    info.width,
                    info.height,
                    format_size(info.bytes)
                )
            },
        );
        div()
            .h(px(33.))
            .px_3()
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .text_size(px(10.))
            .text_color(theme::muted())
            .border_b_1()
            .border_color(theme::border())
            .child(div().flex_1().min_w_0().truncate().child(name))
            .child(details)
            .into_any_element()
    }
}

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            let extension = extension.to_ascii_lowercase();
            Img::extensions().contains(&extension.as_str())
        })
}

fn inspect_image(path: &Path) -> Result<ImageInfo> {
    let metadata =
        fs::metadata(path).with_context(|| format!("无法读取图片 {}", path.display()))?;
    ensure!(metadata.is_file(), "这不是普通文件");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let animated = matches!(extension.as_str(), "gif" | "webp");
    let max_bytes = if animated {
        MAX_ANIMATED_IMAGE_BYTES
    } else {
        MAX_IMAGE_BYTES
    };
    let max_pixels = if animated {
        MAX_ANIMATED_IMAGE_PIXELS
    } else {
        MAX_IMAGE_PIXELS
    };
    ensure!(
        metadata.len() <= max_bytes,
        "图片超过 {}，为避免占用过多内存已停止预览",
        format_size(max_bytes)
    );
    let is_svg = extension == "svg";
    if is_svg {
        return Ok(ImageInfo {
            width: 0,
            height: 0,
            bytes: metadata.len(),
        });
    }
    let reader = ImageReader::open(path)
        .with_context(|| format!("无法解码图片 {}", path.display()))?
        .with_guessed_format()
        .context("无法识别图片格式")?;
    let (width, height) = reader.into_dimensions().context("无法读取图片尺寸")?;
    ensure!(
        u64::from(width) * u64::from(height) <= max_pixels,
        "图片尺寸 {}×{} 过大，为避免占用过多内存已停止预览",
        width,
        height
    );
    Ok(ImageInfo {
        width,
        height,
        bytes: metadata.len(),
    })
}

impl Render for ImagePreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = cx;
        let body = match &self.state {
            PreviewState::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child("正在加载图片…")
                .into_any_element(),
            PreviewState::Error(error) => div()
                .size_full()
                .p_5()
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .text_color(theme::danger())
                .child(error.clone())
                .into_any_element(),
            PreviewState::Ready(info) if info.width == 0 => div()
                .size_full()
                .p_5()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::elevated())
                .child(
                    img(self.path.clone())
                        .object_fit(ObjectFit::Contain)
                        .max_w_full()
                        .max_h_full(),
                )
                .into_any_element(),
            PreviewState::Ready(_) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::background())
                .child(
                    img(self.path.clone())
                        .object_fit(ObjectFit::Contain)
                        .max_w_full()
                        .max_h_full()
                        .with_loading(|| {
                            div()
                                .text_color(theme::muted())
                                .child("正在解码图片…")
                                .into_any_element()
                        })
                        .with_fallback(|| {
                            div()
                                .text_color(theme::danger())
                                .child("图片格式损坏或当前解码器不支持")
                                .into_any_element()
                        }),
                )
                .into_any_element(),
        };
        let info = match &self.state {
            PreviewState::Ready(info) => Some(info),
            _ => None,
        };
        div()
            .id("image-preview")
            .w_full()
            .h(px(self.height))
            .flex_none()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(self.header(info))
            .child(
                div()
                    .h(px((self.height - 33.).max(1.)))
                    .w_full()
                    .min_h_0()
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{inspect_image, is_image_path, MAX_IMAGE_BYTES};
    use crate::ui::format::format_size;
    use std::path::Path;

    #[test]
    fn recognizes_supported_image_extensions_case_insensitively() {
        assert!(is_image_path(Path::new("photo.PNG")));
        assert!(is_image_path(Path::new("icon.svg")));
        assert!(!is_image_path(Path::new("main.rs")));
        assert_eq!(format_size(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn inspects_image_dimensions_without_decoding_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preview.png");
        image::RgbaImage::new(3, 2).save(&path).unwrap();
        let info = inspect_image(&path).unwrap();
        assert_eq!((info.width, info.height), (3, 2));

        let oversized = dir.path().join("oversized.png");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(MAX_IMAGE_BYTES + 1)
            .unwrap();
        assert!(inspect_image(&oversized).is_err());
    }
}
