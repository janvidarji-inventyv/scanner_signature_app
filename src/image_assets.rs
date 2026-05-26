use image;
use std::sync::OnceLock;
use xilem::{Blob, ImageFormat, ImageBrush};
use masonry::peniko::{ImageAlphaType, ImageData};

static ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();
static BULLET_ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();
static SIGNATURE_ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();
static EDIT_SIGNATURE_ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();
static GET_PROCESS_COMPLETE_ICON_CELL: OnceLock<ImageBrush> = OnceLock::new();

pub fn get_icon() -> &'static ImageBrush {
    ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/activate_your_device.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode activate_your_device.png")
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

/// Load the orange bullet icon for step indicators
pub fn get_bullet_icon() -> &'static ImageBrush {
    BULLET_ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/list_point_orange.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode list_point_orange.png")
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
pub fn get_signature_icon() -> &'static ImageBrush {
    SIGNATURE_ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/draw_signature.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode draw_signature.png")
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

pub fn get_edit_signature_icon() -> &'static ImageBrush {
    EDIT_SIGNATURE_ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/edit_signature_black.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode edit_signature_black.png")
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

pub fn get_process_complete_icon() -> &'static ImageBrush {
    GET_PROCESS_COMPLETE_ICON_CELL.get_or_init(|| {
        let bytes = include_bytes!("assets/process_complete.png");
        let img = image::load_from_memory(bytes)
            .expect("Failed to decode process_complete.png")
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