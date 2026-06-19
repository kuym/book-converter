use std::path::{Path, PathBuf};
use image::GenericImageView;

pub struct ImageProcessor;

impl ImageProcessor {
    /// Crops an illustration from the original JPEG based on coordinates provided by the LLM.
    /// The coordinate system is a 0-1000 normalized grid (where 1000 = full width/height).
    pub fn crop_illustration(
        input_path: &Path,
        output_dir: &Path,
        ident: &str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let img = image::open(input_path)?;
        let (orig_w, orig_h) = img.dimensions();

        // Convert from 0-1000 normalized coordinates to actual pixel dimensions
        // We multiply the input units by (dimension / 1000)
        let x1 = ((x as f32 / 1000.0) * orig_w as f32).min(orig_w as f32) as u32;
        let y1 = ((y as f32 / 1000.0) * orig_h as f32).min(orig_h as f32) as u32;
        let x2 = ((x + width) as f32 / 1000.0 * orig_w as f32).min(orig_w as f32) as u32;
        let y2 = ((y + height) as f32 / 1000.0 * orig_h as f32).min(orig_h as f32) as u32;

        // Ensure that the width and height are at least 1 pixel to avoid empty view
        let final_x2 = x2.max(x1 + 1).min(orig_w);
        let final_y2 = y2.max(y1 + 1).min(orig_h);

        // Calculate the dimensions for the view
        let final_width = final_x2.saturating_sub(x1);
        let final_height = final_y2.saturating_sub(y1);

        if final_width > 0 && final_height > 0 {
            let sub_image = img.view(x1 as u32, y1 as u32, final_width, final_height).to_image();
            
            if !output_dir.exists() {
                std::fs::create_dir_all(output_dir)?;
            }

            let filename = format!("{}_crop_{}.jpg", ident, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs());
            let output_path = output_dir.join(filename);
            sub_image.save(&output_path)?;
            Ok(output_path)
        } else {
            // Fallback for invalid dimensions to avoid crashing the batch process
            Err("Invalid crop coordinates: resulting width or height is zero.".into())
        }
    }
}
