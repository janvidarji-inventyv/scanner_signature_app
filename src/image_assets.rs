use image;
use std::sync::OnceLock;
use xilem::{Blob, ImageFormat, ImageBrush};
use masonry::peniko::{ImageAlphaType, ImageData};

static ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();

pub fn get_icon() -> &'static ImageBrush {
    ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/activate_your_device.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode icon.png")
            .into_rgba8();
        let width = img.width();
        let height = img.height();
        let data = img.into_raw();
        let image_data = ImageData {
            data: Blob::new(std::sync::Arc::new(data)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width,
            height,
        };
        ImageBrush::new(image_data)
    })
}