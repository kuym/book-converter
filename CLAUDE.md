# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Rust CLI (`book-converter`) that turns a JPEG photo of an open book's pages into an HTML
reconstruction, using a **render → compare → refine** convergence loop driven by a local
multimodal LLM (default `gemma4:e4b` via Ollama). Target use case (see `prompt.txt`): a special
e-reader for children with learning differences.

Each iteration it: (1) (re-)detects & crops the illustrations from the photo — correcting the prior
round's bounding boxes from crop feedback, (2) asks the LLM to author/refine a complete standalone
HTML page reproducing the photo, (3) renders that HTML in headless Chrome and dumps a PNG
screenshot, (4) feeds the original photo + the screenshot back to the LLM to get a similarity
score, a list of layout differences, and per-illustration crop adjustments (where a crop is cut off
or over-inclusive), then loops — refining the HTML from the differences and the crops from the crop
adjustments — until the model converges (or `similarity >= threshold`) or `--max-iterations` is
hit. Each iteration is written to its own `iteration_NN/` subdirectory so a human can inspect every
step. Both the HTML and the illustration crops improve progressively across iterations.

## Commands

```bash
cargo build                    # debug build -> target/debug/book-converter
cargo build --release
cargo run -- --input test/seuss-1.jpg                          # run against the sample image
cargo run -- -i <img.jpg> -o ./output --max-iterations 5 --threshold 90
cargo test                     # no tests exist yet
```

CLI flags (`src/main.rs`): `-i/--input` (required JPEG), `-o/--output-dir` (default `./output`),
`-m/--model` (default `gemma4:e4b`), `--ollama-url` (default `http://localhost:11434`),
`--max-iterations` (default 5), `--threshold` (default 90; stop when similarity ≥ this).

Runtime requirements:
- A running Ollama instance serving the chosen multimodal model.
- A browser for headless rendering. The `fetch` feature is enabled, so if no system
  Chrome/Chromium/Edge is found, `headless_chrome` auto-downloads a Chromium build on first run
  (cached). A system browser, if present, is preferred (`renderer.rs` calls `default_executable`).

Output: `output/iteration_NN/` per iteration, each containing `page.html`, `render.png` (the
headless-Chrome screenshot), the cropped illustration JPEGs, and `critique.json` (the model's
similarity score + differences). Only `target` and `.DS_Store` are gitignored.

## Architecture

`main.rs` orchestrates a convergence loop. The structured-extraction + deterministic-HTML approach
was replaced by **direct LLM HTML authorship with a visual feedback loop** — the model writes the
HTML, renders are screenshotted, and the model critiques its own render against the original.

- `ollama.rs` — `OllamaClient` wraps Ollama `/api/chat`. One private `chat(prompt, images, json)`
  helper (base64-encodes images, optional `format: "json"`) backs four methods:
  `detect_illustrations` (JSON → bounding boxes; takes the previous boxes + crop feedback and
  refines them in place, keeping identifiers stable), `generate_html` (raw HTML), `compare`
  (two images + the current crops → JSON `Critique`, judging both layout and crop quality),
  `refine_html` (raw HTML). HTML responses are de-fenced by `strip_html_fences`; JSON responses
  are salvaged by `extract_json`.
- `renderer.rs` — `Renderer` holds one long-lived headless-Chrome `Browser` (idle timeout raised
  to 1h so it survives multi-minute LLM calls between renders) and screenshots `page.html` via a
  `file://` URL.
- `image_processor.rs` — `crop_all` decodes the original once and returns in-memory
  `CroppedAsset { filename, description, bytes }` per illustration, written into every iteration
  dir under a stable, sanitized `{identifier}.jpg` name that the generated HTML references.
- `models.rs` — `IllustrationInfo` (the **0–1000 normalized coordinate grid** for crop boxes),
  `AnalysisResponse`, and `Critique { similarity, converged, differences, crop_adjustments }`.

`main.rs` carries four pieces of state between iterations: `prev_html` (to refine), `differences`
(layout fixes), `crop_feedback` (→ next detection), and `prev_illustrations` (boxes to correct).

Key contracts: the LLM must reference illustrations by the exact cropped filenames it's given in
the prompt, and `detect_illustrations` keeps each illustration's `identifier` (hence filename)
stable across iterations so the refined HTML's `<img src>` stays valid while the crop tightens; the
render window size (`page_dimensions` in `main.rs`, width fixed at 1000px, height scaled to the
photo's aspect ratio) is shared between the prompt and the screenshot so comparisons line up.
`prompt.txt` is the original project spec, not a runtime input.

## Known limitations

- Screenshots capture the viewport only (no `capture_beyond_viewport`); the prompt asks the model
  to fit the page within the window, so very tall pages could clip.
- Convergence relies entirely on the model's self-reported `similarity`/`converged` — there is no
  pixel-level image diff.
