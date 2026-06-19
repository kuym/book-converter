use std::fs;
use reqwest::Client;
use serde_json::Value;
use crate::models::AnalysisResponse;

pub struct OllamaClient {
    url: String,
    model: String,
    client: Client,
}

impl OllamaClient {
    pub fn new(api_url: &str, model: &str) -> Self {
        let base_url = if api_url.ends_with('/') {
            api_url.to_string()
        } else {
            format!("{}/", api_url)
        };
        Self {
            url: format!("{}/api/chat", base_url),
            model: model.to_string(),
            client: Client::new(),
        }
    }

    pub async fn analyze_page(&self, image_path: &str, prompt: &str) -> Result<AnalysisResponse, Box<dyn std::error::Error>> {
        let img_data = fs::read(image_path)?;
        let base64_img = base64::encode(&img_data);

        // Prepare the actual payload for transmission
        let payload = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt,
                    "images": [base64_img]
                }
            ],
            "stream": false,
            "format": "json"
        });

        // Debugging: Log and isolate image data specifically as requested
        println!("--- Request Metadata ---");
        println!("Model: {}", self.model);
        println!("Prompt excerpt: {}...", &prompt[..std::cmp::min(prompt.len(), 5000)]);
        println!("Image Data (First 100 chars): {}...", &base64_img[..std::cmp::min(base64_img.len(), 100)]);
        println!("------------------------");

        let response = self.client.post(&self.url)
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        println!("DEBUG: Raw Ollama Response:\n{}", response);

        let json_response: Value = serde_json::from_str(&response)?;

        if let Some(err) = json_response.get("error") {
            return Err(format!("Ollama returned an error: {}", err).into());
        }

        let content = json_response["message"]["content"]
            .as_str()
            .ok_or("Missing 'message.content' field in Ollama response")?;

        let analysis: AnalysisResponse = serde_json::from_str(content.trim())?;
        Ok(analysis)
    }
}
