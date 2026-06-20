# Book-Converter

A high-fidelity, agentic command-line utility written in Rust designed to transform photographs of open book pages into structurally accurate, interactive HTML documents. 

This tool is specifically engineered for use cases like creating special e-readers for children, where the goal is to preserve the original layout, typography, and illustrations of a physical book in a digital, web-based format.

## 🚀 Overview

Unlike traditional OCR tools that simply extract text, **book-converter** uses an iterative "Refinement Loop" powered by multi-modal Large Language Models (served locally via Ollama). The application doesn't just guess the layout; it actively observes its own work, compares a rendered screenshot of the generated HTML against the original photograph, and performs a visual critique to correct errors in text, positioning, and image cropping.

### The Iterative Convergence Loop

The utility operates through a sophisticated feedback loop:
1.  **Detection:** An LLM analyzes the input JPEG to identify illustrations and decorative elements using normalized coordinates.
2.  **Extraction:** The tool crops these identified illustrations from the original photo and saves them as individual assets.
3.  **Generation:** The LLM generates a complete, standalone HTML/CSS document, performing OCR and reconstructing the page layout.
4.  **Rendering:** A headless Chrome instance renders the generated HTML into a screenshot.
5.  **Critique & Comparison:** The LLM compares the *original photo* with the *newly rendered screenshot*. It identifies discrepancies (e.g., "the text is too small," "the image is cut off") and generates a `critique.json`.
6.  **Refinement:** If the similarity threshold isn't met, the tool restarts the loop, feeding the previous HTML and the critique back into the LLM to "fix" the page.

## Key Features

*   **Agentic Self-Correction:** Uses visual feedback to iteratively improve layout fidelity.
*   **Multi-Modal Integration:** Seamlessly communicates with local Ollama instances (optimized for models like `gemma4`).
*   **Automated Asset Management:** Automatically detects, crops, and manages illustration assets (`.jpg`) so they are correctly referenced in the output HTML.
*   **Headless Rendering Engine:** Utilizes `headless-chrome` to provide a "ground truth" visual comparison during the refinement process.
*   **High Fidelity:** Aims for pixel-perfect reproduction of columns, headings, font weights, and spacing.

## Prerequisites

Before running the utility, ensure you have the following installed:

*   **Rust Toolchain:** `cargo` and `rustc`.
*   **Ollama:** A running instance of Ollama with a multi-modal model (e.g. `gemma4:26b` or `gemma4:e4b`) pulled and ready.
*   **Google Chrome or Chromium:** Required for the headless rendering/screenshotting phase.
*   **System Dependencies:** Standard build tools (linker, etc.) required for compiling Rust crates like `headless_chrome`.

## Getting Started

### Installation

Clone the repository and build the project using Cargo:

```bash
git clone https://github.com/kuym/book-converter.git
cd book-converter
cargo build --release
```

### Usage

Run the utility by passing an input JPEG of a book page.

```bash
./target/release/book-converter --input ./my_book_page.jpg --output_dir ./output
```

### Command Line Arguments

| Argument | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--input` | `-i` | *Required* | Path to the input JPEG photo of an open book page. |
| `--output-dir` | `-o` | `./output` | The directory where iterations and assets will be saved. |
  | `--model` | `-m` | `gemma4:e4b` | The specific multi-modal model to use in Ollama. |
| `--ollama-url` | | `http://localhost:11434` | The API base URL for your local Ollama instance. |
| `--max-iterations`| | `5` | Maximum number of refinement loops to perform. |
| `--threshold` | | `90` | Similarity score (0-100) required to stop the loop early. |

## 📂 Output Structure

Each execution creates a series of subdirectories within your output folder, representing each stage of the refinement process:

```text
output/
└── iteration_01/
    ├── page.html         <-- The generated HTML document
    ├── render.png        <-- A screenshot of what the HTML looks like
    ├── critique.json     <-- The LLM's analysis of differences
    ├── illustration_1.jpg <-- Extracted image asset
    └── illustration_2.jpg <-- Extracted image asset
```

## Technical Architecture

*   **`main.rs`**: Orchestrates the state machine, managing the loop between `OllamaClient`, `ImageProcessor`, and `Renderer`.
*   **`ollama.rs`**: Handles multi-modal prompt engineering, including base64 image encoding for the Ollama API and JSON parsing of model responses.
*   **`image_processor.rs`**: Uses the `image` crate to perform precise sub-region cropping based on LLM-detected coordinates.
*   **`renderer.rs`**: Manplements a long-lived `headless-chrome` instance to capture high-resolution screenshots of rendered HTML.
*   **`models.rs`**: Defines the shared data schema (Serde) for communication between the Rust logic and the LLM's JSON output.

## Important Notes

*   **Model Choice:** This utility relies heavily on the multi-modal capabilities of the LLM. For best results, use a model capable of high-quality visual reasoning and JSON instruction following (like Gemma 4).
*   **Performance:** The iterative loop can be computationally expensive and slow, as it requires multiple LLM inferences and browser renders per page.
*   **First Run:** On the first run, the utility may attempt to download a Chromium build if a system-installed version is not detected.

