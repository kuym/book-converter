use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum BlockType {
    Text,
    Subheading,
    Caption,
    Illustration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentBlock {
    pub block_type: BlockType,
    pub content: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IllustrationInfo {
    pub description: String,
    pub identifier: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResponse {
    pub blocks: Vec<ContentBlock>,
    pub illustrations: Vec<IllustrationInfo>,
}
