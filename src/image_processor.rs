use crate::models::IllustrationInfo;
use image::{DynamicImage, GenericImageView};
use std::io::Cursor;

/// An illustration cropped out of the original photo, held in memory so it can
/// be written into every iteration's output directory.
pub struct CroppedAsset {
    /// Filename the HTML references it by, e.g. `cat_1.jpg`.
    pub filename: String,
    pub description: String,
    /// Encoded JPEG bytes.
    pub bytes: Vec<u8>,
}

pub struct ImageProcessor;

impl ImageProcessor {
    /// Crop every detected illustration out of the original image (decoded
    /// once) and return them as encoded JPEGs. Illustrations whose coordinates
    /// resolve to an empty region are skipped rather than failing the run.
    pub fn crop_all(
        original: &DynamicImage,
        illustrations: &[IllustrationInfo],
    ) -> Vec<CroppedAsset> {
        let mut assets = Vec::new();
        for ill in illustrations {
            match Self::crop_one(original, ill) {
                Ok(asset) => assets.push(asset),
                Err(e) => eprintln!(
                    "  ! skipping illustration '{}': {}",
                    ill.identifier, e
                ),
            }
        }
        assets
    }

    fn crop_one(
        original: &DynamicImage,
        ill: &IllustrationInfo,
    ) -> Result<CroppedAsset, Box<dyn std::error::Error>> {
        let (orig_w, orig_h) = original.dimensions();

        // Convert 0-1000 normalized coordinates to actual pixels.
        let to_px = |v: u32, dim: u32| ((v as f32 / 1000.0) * dim as f32) as u32;
        let x1 = to_px(ill.x, orig_w).min(orig_w.saturating_sub(1));
        let y1 = to_px(ill.y, orig_h).min(orig_h.saturating_sub(1));
        let x2 = to_px(ill.x + ill.width, orig_w).min(orig_w);
        let y2 = to_px(ill.y + ill.height, orig_h).min(orig_h);

        let crop_w = x2.saturating_sub(x1);
        let crop_h = y2.saturating_sub(y1);
        if crop_w == 0 || crop_h == 0 {
            return Err("resulting crop has zero width or height".into());
        }

        let sub = original.view(x1, y1, crop_w, crop_h).to_image();

        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(sub)
            .to_rgb8()
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)?;

        Ok(CroppedAsset {
            filename: format!("{}.jpg", sanitize(&ill.identifier)),
            description: ill.description.clone(),
            bytes,
        })
    }
}

/// Keep illustration identifiers safe to use as filenames / `src` attributes.
fn sanitize(ident: &str) -> String {
    let cleaned: String = ident
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "illustration".to_string()
    } else {
        trimmed.to_string()
    }
}
