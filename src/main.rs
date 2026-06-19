use clap::Parser;

mod image_processor;
mod models;
mod ollama;
mod renderer;

use crate::image_processor::{CroppedAsset, ImageProcessor};
use crate::ollama::OllamaClient;
use crate::renderer::Renderer;

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input JPEG photo of an open book page
    #[arg(short, long)]
    input: String,

    /// Output directory; each iteration is written to its own subfolder here
    #[arg(short, long, default_value = "./output")]
    output_dir: String,

    /// Multimodal model served by Ollama
    #[arg(short, long, default_value = "gemma4:e4b")]
    model: String,

    /// Ollama API base URL
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Maximum number of refinement iterations
    #[arg(long, default_value_t = 5)]
    max_iterations: u32,

    /// Similarity score (0-100) at or above which we consider the page converged
    #[arg(long, default_value_t = 90)]
    threshold: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !Path::new(&args.input).exists() {
        eprintln!("Error: Input file {} does not exist.", args.input);
        std::process::exit(1);
    }

    println!("Input file:       {}", args.input);
    println!("Model:            {}", args.model);
    println!("Output directory: {}", args.output_dir);
    println!("Max iterations:   {}", args.max_iterations);

    // Original photo: keep the raw bytes for the LLM, and a decoded copy for cropping.
    let original_bytes = fs::read(&args.input)?;
    let original_img = image::load_from_memory(&original_bytes)
        .map_err(|e| format!("failed to decode input image: {}", e))?;
    let page_px = page_dimensions(&original_img);
    println!("Render size:      {}x{}", page_px.0, page_px.1);

    let client = OllamaClient::new(&args.ollama_url, &args.model);

    // --- Launch the headless browser used to screenshot each render. ---
    let renderer = Renderer::new(page_px.0, page_px.1)?;

    fs::create_dir_all(&args.output_dir)?;

    // State carried between iterations: the previous HTML to refine, the layout
    // differences and crop adjustments from the last critique, and the previous
    // illustration boxes so detection can correct them rather than start over.
    let mut prev_html: Option<String> = None;
    let mut differences: Vec<String> = Vec::new();
    let mut crop_feedback: Vec<String> = Vec::new();
    let mut prev_illustrations: Vec<models::IllustrationInfo> = Vec::new();
    let mut last_dir: Option<PathBuf> = None;

    for iteration in 1..=args.max_iterations {
        let iter_dir = PathBuf::from(&args.output_dir).join(format!("iteration_{:02}", iteration));
        fs::create_dir_all(&iter_dir)?;
        last_dir = Some(iter_dir.clone());

        // --- Re-detect & re-crop illustrations, correcting last round's boxes. ---
        println!("[iteration {}] Detecting & cropping illustrations...", iteration);
        let illustrations = match client
            .detect_illustrations(&original_bytes, &prev_illustrations, &crop_feedback)
            .await
        {
            Ok(list) => list,
            Err(e) => {
                eprintln!("  ! illustration detection failed ({e}); reusing previous boxes");
                prev_illustrations.clone()
            }
        };
        let assets = ImageProcessor::crop_all(&original_img, &illustrations);
        println!("  using {} illustration(s)", assets.len());
        let asset_meta: Vec<(String, String)> = assets
            .iter()
            .map(|a| (a.filename.clone(), a.description.clone()))
            .collect();

        // --- Generate (first pass) or refine (later passes) the HTML. ---
        let html = match &prev_html {
            None => {
                println!("[iteration {}] Generating initial HTML...", iteration);
                client
                    .generate_html(&original_bytes, &asset_meta, renderer.window_size())
                    .await?
            }
            Some(prev) => {
                println!("[iteration {}] Refining HTML...", iteration);
                client
                    .refine_html(
                        &original_bytes,
                        prev,
                        &differences,
                        &asset_meta,
                        renderer.window_size(),
                    )
                    .await?
            }
        };

        // Write the HTML and the (stable-named) illustration assets next to it.
        let html_path = iter_dir.join("page.html");
        fs::write(&html_path, &html)?;
        write_assets(&iter_dir, &assets)?;

        // Render and dump a PNG screenshot for debugging / comparison.
        println!("[iteration {}] Rendering in headless Chrome...", iteration);
        let png = renderer.screenshot(&html_path)?;
        let png_path = iter_dir.join("render.png");
        fs::write(&png_path, &png)?;
        println!("  wrote {}", iter_dir.display());

        // Compare against the original, judging layout AND illustration crops.
        println!("[iteration {}] Comparing render to original...", iteration);
        let critique = match client.compare(&original_bytes, &png, &illustrations).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  ! comparison failed: {e}; stopping");
                break;
            }
        };
        fs::write(
            iter_dir.join("critique.json"),
            serde_json::to_string_pretty(&critique)?,
        )?;

        println!(
            "  similarity: {}/100, model-converged: {}",
            critique.similarity, critique.converged
        );
        for d in &critique.differences {
            println!("    - {}", d);
        }
        for c in &critique.crop_adjustments {
            println!("    [crop] {}", c);
        }

        if critique.converged || critique.similarity >= args.threshold {
            println!(
                "\nConverged at iteration {} (similarity {} >= threshold {}).",
                iteration, critique.similarity, args.threshold
            );
            break;
        }

        if iteration == args.max_iterations {
            println!(
                "\nReached max iterations ({}) without converging.",
                args.max_iterations
            );
            break;
        }

        // Carry state into the next iteration.
        prev_html = Some(html);
        differences = critique.differences;
        crop_feedback = critique.crop_adjustments;
        prev_illustrations = illustrations;
    }

    if let Some(dir) = last_dir {
        println!("\nDone. Final output: {}", dir.display());
    }
    Ok(())
}

/// Standardize the render width to 1000px and scale height to preserve the
/// original photo's aspect ratio.
fn page_dimensions(img: &image::DynamicImage) -> (u32, u32) {
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return (1000, 1400);
    }
    let target_w = 1000u32;
    let target_h = ((target_w as f32) * (h as f32) / (w as f32)).round() as u32;
    (target_w, target_h.max(1))
}

/// Write each cropped illustration into the iteration directory under the
/// filename the HTML references it by.
fn write_assets(dir: &Path, assets: &[CroppedAsset]) -> Result<(), Box<dyn std::error::Error>> {
    for asset in assets {
        fs::write(dir.join(&asset.filename), &asset.bytes)?;
    }
    Ok(())
}
