use crate::models::{AnalysisResponse, Critique, IllustrationInfo};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct OllamaClient {
    url: String,
    model: String,
    client: Client,
}

impl OllamaClient {
    pub fn new(api_url: &str, model: &str) -> Self {
        let base_url = api_url.trim_end_matches('/');
        Self {
            url: format!("{}/api/chat", base_url),
            model: model.to_string(),
            // Local multimodal generation can be slow; allow generous time.
            client: Client::builder()
                .timeout(Duration::from_secs(600))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Low-level single-turn chat call. `images` are raw bytes that get
    /// base64-encoded into the message. When `json` is true the model is
    /// asked to emit a JSON object; otherwise free-form text is returned.
    async fn chat(
        &self,
        prompt: &str,
        images: &[&[u8]],
        json: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let encoded: Vec<String> = images.iter().map(|b| base64::encode(b)).collect();

        let mut message = serde_json::json!({
            "role": "user",
            "content": prompt,
        });
        if !encoded.is_empty() {
            message["images"] = serde_json::json!(encoded);
        }

        let mut payload = serde_json::json!({
            "model": self.model,
            "messages": [message],
            "stream": false,
            "options": { "temperature": 0.2 },
        });
        if json {
            payload["format"] = serde_json::json!("json");
        }

        let response = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        let json_response: Value = serde_json::from_str(&response)
            .map_err(|e| format!("Ollama returned non-JSON response: {} ({})", e, response))?;

        if let Some(err) = json_response.get("error") {
            return Err(format!("Ollama returned an error: {}", err).into());
        }

        let content = json_response["message"]["content"]
            .as_str()
            .ok_or("Missing 'message.content' field in Ollama response")?;

        Ok(content.to_string())
    }

    /// Detect illustration regions on the original page so they can be cropped
    /// out of the photo and re-used as `<img>` assets in the generated HTML.
    ///
    /// Re-run every iteration: when `previous` boxes and `crop_feedback` are
    /// supplied, the model adjusts the bounding boxes to fix cropping problems
    /// (parts cut off, surrounding text/background included) rather than
    /// detecting from scratch, keeping each illustration's identifier stable.
    pub async fn detect_illustrations(
        &self,
        image: &[u8],
        previous: &[IllustrationInfo],
        crop_feedback: &[String],
    ) -> Result<Vec<IllustrationInfo>, Box<dyn std::error::Error>> {
        let base = r#"You are analyzing a photograph of an open book page for a children's e-reader.
Identify every illustration, drawing, photo or decorative image on the page (ignore plain text).
For each one give a bounding box that tightly contains the WHOLE picture without cutting any part
of it off and without including surrounding text or page background.
Return ONLY a JSON object of the form:
{
  "illustrations": [
    {
      "description": "short description of the picture",
      "identifier": "unique_snake_case_tag",
      "x": 0, "y": 0, "width": 1000, "height": 1000
    }
  ]
}
Coordinates use a 0-1000 grid where x,y is the top-left corner and width,height the size,
both relative to the full image (1000 == full width or height). If there are no illustrations,
return {"illustrations": []}."#;

        let prompt = if previous.is_empty() {
            base.to_string()
        } else {
            let prev_json = serde_json::to_string_pretty(previous)
                .unwrap_or_else(|_| "[]".to_string());
            let feedback = if crop_feedback.is_empty() {
                "(no specific feedback; tighten any boxes that cut off part of a picture)"
                    .to_string()
            } else {
                crop_feedback
                    .iter()
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                r#"{base}

This is a REFINEMENT pass. Here are the bounding boxes used last time:
{prev}

Feedback on how those crops looked once rendered (a crop that is too small cuts off part of the
picture; a crop that is too large includes page text or background). Adjust x/y/width/height to fix
these problems and capture each whole illustration cleanly:
{feedback}

Return the corrected regions. Keep the SAME "identifier" for the same illustration so its filename
stays stable; only change coordinates. Add or remove regions only if the previous set was wrong."#,
                base = base,
                prev = prev_json,
                feedback = feedback,
            )
        };

        let content = self.chat(&prompt, &[image], true).await?;
        let analysis: AnalysisResponse = serde_json::from_str(extract_json(&content))?;
        Ok(analysis.illustrations)
    }

    /// First-pass HTML: ask the model to reproduce the page from the photo.
    pub async fn generate_html(
        &self,
        image: &[u8],
        assets: &[(String, String)],
        page_px: (u32, u32),
    ) -> Result<String, Box<dyn std::error::Error>> {
        let prompt = format!(
            r#"You are building a faithful HTML reproduction of the book page in the image, for a
children's e-reader. Transcribe (OCR) all of the text exactly, and recreate the layout, headings,
captions, columns, colours, fonts and spacing as closely as you can.

{assets}

Requirements:
- Output a COMPLETE standalone HTML document (<!DOCTYPE html> ... </html>).
- Put all CSS inline in a single <style> block in the <head>. Do not link external resources.
- Size the page to {w}px wide by {h}px tall (the body should fill it with no scrollbars).
- Reference illustrations only via the exact filenames listed above, e.g. <img src="cat_1.jpg">,
  positioning and sizing each one to match where it sits on the original page.
- Reproduce the text content verbatim.

Respond with ONLY the raw HTML. No markdown fences, no commentary."#,
            assets = describe_assets(assets),
            w = page_px.0,
            h = page_px.1,
        );

        let content = self.chat(&prompt, &[image], false).await?;
        Ok(strip_html_fences(&content))
    }

    /// Visually compare the original photo (first image) against a screenshot
    /// of the current render (second image). `illustrations` describes the crops
    /// currently embedded in the render so the model can judge whether any are
    /// cut off or over-inclusive and suggest crop adjustments.
    pub async fn compare(
        &self,
        original: &[u8],
        render: &[u8],
        illustrations: &[IllustrationInfo],
    ) -> Result<Critique, Box<dyn std::error::Error>> {
        let illustration_list = if illustrations.is_empty() {
            "There are no cropped illustrations in this reproduction.".to_string()
        } else {
            let mut s = String::from(
                "The reproduction embeds these illustrations, each cropped from the original photo:\n",
            );
            for ill in illustrations {
                s.push_str(&format!("- {} : {}\n", ill.identifier, ill.description));
            }
            s
        };

        let prompt = format!(
            r#"You are reviewing an HTML reproduction of a book page.
The FIRST image is the original book page photograph (the target).
The SECOND image is a screenshot of the HTML reproduction so far.
Compare them and report how well the reproduction matches the original.

{illustration_list}
Look closely at each embedded illustration: compare it to the same picture in the original photo
and decide whether the crop cuts off part of the picture (too small) or includes unwanted text or
background (too large). Provide a "crop_adjustments" note for any illustration that needs a better
crop, naming it by its identifier and saying which way to extend or tighten the box.

Return ONLY a JSON object of the form:
{{
  "similarity": 0,
  "converged": false,
  "differences": ["specific, actionable layout/text/style difference to fix", "..."],
  "crop_adjustments": ["identifier: extend the box downward to include the cup's base", "..."]
}}
"similarity" is 0-100 (100 == visually indistinguishable) and should account for illustrations that
are cut off. Set "converged" to true only when the reproduction is an excellent match AND every
illustration is cleanly cropped. List the most important items first; use an empty array when there
is nothing to report for a field."#,
            illustration_list = illustration_list,
        );

        let content = self.chat(&prompt, &[original, render], true).await?;
        let critique: Critique = serde_json::from_str(extract_json(&content))?;
        Ok(critique)
    }

    /// Produce an improved HTML document given the previous attempt and the
    /// list of differences found by `compare`.
    pub async fn refine_html(
        &self,
        image: &[u8],
        current_html: &str,
        differences: &[String],
        assets: &[(String, String)],
        page_px: (u32, u32),
    ) -> Result<String, Box<dyn std::error::Error>> {
        let diff_list = if differences.is_empty() {
            "(no specific notes were provided; improve the overall fidelity)".to_string()
        } else {
            differences
                .iter()
                .map(|d| format!("- {}", d))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt = format!(
            r#"You are improving an HTML reproduction of the book page shown in the image, for a
children's e-reader. Below is your current HTML and a list of differences between its rendered
screenshot and the original photo. Produce an improved version that fixes these differences and
matches the original page more closely.

Differences to fix:
{diffs}

{assets}

Requirements:
- Output a COMPLETE standalone HTML document with all CSS inline in a single <style> block.
- Size the page to {w}px wide by {h}px tall, body fills it with no scrollbars.
- Reference illustrations only via the exact filenames listed above.
- Keep the transcribed text accurate to the original image.

Current HTML:
{html}

Respond with ONLY the raw improved HTML. No markdown fences, no commentary."#,
            diffs = diff_list,
            assets = describe_assets(assets),
            w = page_px.0,
            h = page_px.1,
            html = current_html,
        );

        let content = self.chat(&prompt, &[image], false).await?;
        Ok(strip_html_fences(&content))
    }
}

/// Render the available illustration assets as a prompt-friendly list.
fn describe_assets(assets: &[(String, String)]) -> String {
    if assets.is_empty() {
        return "There are no extracted illustration files available for this page.".to_string();
    }
    let mut s = String::from("Available illustration files (already cropped from the original photo, located in the same folder as the HTML):\n");
    for (filename, description) in assets {
        s.push_str(&format!("- {} : {}\n", filename, description));
    }
    s
}

/// Models sometimes wrap HTML in ```html ... ``` fences despite instructions;
/// strip them and any leading prose before the first tag.
fn strip_html_fences(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("```html") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("```") {
        s = rest;
    }
    if let Some(idx) = s.rfind("```") {
        s = &s[..idx];
    }
    let s = s.trim();
    // Drop any leading commentary before the document/markup actually starts.
    if let Some(idx) = s.find("<!DOCTYPE").or_else(|| s.find("<html")) {
        return s[idx..].trim().to_string();
    }
    s.to_string()
}

/// Extract the first balanced JSON object from a model response. Tolerates
/// stray prose or fences around the JSON.
fn extract_json(raw: &str) -> &str {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => &raw[s..=e],
        _ => raw.trim(),
    }
}
