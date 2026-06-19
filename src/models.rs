use serde::{Deserialize, Serialize};

/// A single illustration region detected on the page, expressed in the
/// 0-1000 normalized coordinate grid (where 1000 == full width/height).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IllustrationInfo {
    pub description: String,
    pub identifier: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Result of the illustration-detection pass over the original photo.
/// Only `illustrations` is required; any text blocks the model volunteers
/// are accepted but unused (the HTML generator reads text from the image).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnalysisResponse {
    #[serde(default)]
    pub illustrations: Vec<IllustrationInfo>,
}

/// The model's visual comparison of the original photo against a screenshot
/// of the rendered HTML. Drives the convergence decision.
#[derive(Debug, Serialize, Deserialize)]
pub struct Critique {
    /// Overall visual similarity, 0 (nothing alike) to 100 (indistinguishable).
    #[serde(default)]
    pub similarity: u8,
    /// The model's own judgement that the render is good enough to stop.
    #[serde(default)]
    pub converged: bool,
    /// Concrete, actionable differences to fix in the next iteration.
    #[serde(default)]
    pub differences: Vec<String>,
    /// Per-illustration notes on how the crop should change (extend to include
    /// a cut-off part, tighten to exclude surrounding text/background, etc.).
    /// These feed the next iteration's illustration re-detection.
    #[serde(default)]
    pub crop_adjustments: Vec<String>,
}
