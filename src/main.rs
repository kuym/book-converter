use clap::Parser;

mod ollama;
mod image_processor;
mod page_renderer;
mod models;

use crate::ollama::OllamaClient;
use crate::page_renderer::PageRenderer;
use crate::image_processor::ImageProcessor;

use std::path::{Path, PathBuf};
use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input JPEG image of a book page
    #[arg(short, long)]
    input: String,

    /// Output directory for the images and HTML files
    #[arg(short, long, default_value = "./output")]
    output_dir: String,

    /// Model to use (e.g., gemma4:12b-mlx)
    #[arg(short, long, default_value = "gemma4:e4b")]
    model: String,

    /// Ollama API base URL
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    if !Path::new(&args.input).exists() {
        eprintln!("Error: Input file {} does not exist.", args.input);
        std::process::exit(1);
    }

    println!("Input File: {}", args.input);
    println!("Model: {}", args.model);
    println!("Output Directory: {}", args.output_dir);

    let client = OllamaClient::new(&args.ollama_url, &args.model);
    
    let prompt = r#"
        Analyze this book page image and extract its structure for reconstruction as a web page.
        Return a JSON object containing:
        1. A list of content blocks (Text, Subheading, Caption). Each block should have:
           - block_type (string: 'Text', 'Subheading', or 'Caption')
           - content (the actual text)
           - x, y, width, height (integers from 0 to 1000 representing position on a grid relative to the image size).
        2. A list of illustrations. Each illustration should have:
           - description (string)
           - identifier (unique string tag)
           - x, y, width, height (integers from 0 to 1000)

        Example JSON format:
        {
          "blocks": [
            {"block_type": "Subheading", "content": "Chapter One", "x": 50, "y": 40, "width": 900, "height": 100}
          ],
          "illustrations": [
            {"description": "a drawing of a cat", "identifier": "cat_1", "x": 200, "y": 300, "width": 400, "height": 400}
          ]
        }
    "#;

    let mut iteration = 0;
    let max_iterations = 3;
    
    while iteration < max_iterations {
        println!("\n--- Iteration {} ---", iteration + 1);
        
        match client.analyze_page(&args.input, prompt).await {
            Ok(analysis) => {
                println!("Analysis received.");

                if !Path::new(&args.output_dir).exists() {
                    fs::create_dir_all(Path::new(&args.output_dir))?;
                }

                for ill in &analysis.illustrations {
                    println!("Extracting illustration: {}", ill.identifier);
                    let _ = ImageProcessor::crop_illustration(
                        &Path::new(&args.input),
                        Path::new(&args.output_dir),
                        &ill.identifier,
                        ill.x,
                        ill.y,
                        ill.width,
                        ill.height,
                    );
                }

                let html = PageRenderer::render(&analysis);
                let output_html_path = PathBuf::from(&args.output_dir).join("page.html");
                fs::write(&output_html_path, html)?;
                println!("Rendered successfully to: {:?}", output_html_path);

                if iteration == max_iterations - 1 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error during analysis: {}", e);
                break;
            }
        }

        iteration += 1;
    }

    Ok(())
}
