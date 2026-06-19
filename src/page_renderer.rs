use crate::models::{AnalysisResponse, BlockType};

pub struct PageRenderer;

impl PageRenderer {
    /// Renders the AnalysisReport into a standalone HTML page including CSS for layout.
    pub fn render(analysis: &AnalysisResponse) -> String {
        let mut style = String::new();
        style.push_str("
            body { 
                background-color: #f4f4f4; 
                font-family: 'Georgia', serif; 
                display: flex; justify-content: center; align-items: center; 
                margin: 0; padding: 20px; height: 100vh; overflow: auto;
            }
            .page {
                background-color: white;
                width: 800px;
                height: calc(var(--_page-aspect-ratio) * var(--page-width)); /* This is conceptually, but we'll stick to dynamic dimensions */
                margin: auto;
                position: relative;
                box-shadow: 0 10px 35px rgba(0,0,0,0.2);
                padding: 40px;
                border-radius: 4px;
            }
            /* Since we don't know the exact aspect ratio from just a list of blocks, we use the bounding area */
            .page { 
                width: 800px;
                height: 1200px; /* Default book-like proportions */
            }
            .block {
                position: absolute;
                box-sizing: border-box;
                max-width: 90%;
            }
            .text { font-size: 1.1rem; line-height: 1.6; color: #333; text-align: justify; }
            .subheading { font-size: 1.5rem; font-weight: bold; margin-bottom: 10px; color: #000; line-height: 1.2; }
            .caption { font-style: italic; font-size: 0.9rem; color: #666; }
            .illustration { border: 1px solid #ccc; background-color: #f9f9f9; display: block; }
        ");

        let mut content = String::new();
        for block in &analysis.blocks {
            let style_class = match block.block_type {
                BlockType::Text => "text",
                BlockType::Subheading => "subheading",
                BlockType::Caption => "caption",
                BlockType::Illustration => "illustration",
            };

            // Converting 0-1000 grid to % values for CSS.
            // The layout assumes a 1:1 mapping of the coordinate space.
            let style = format!(
                "top:{}%; left:{}%; width:{}%; height:{}%;",
                block.y, block.x, block.width, block.height
            );

            content.push_str(&format!(
                r#"<div class="block {}" style="{}">{}{}</div>"#,
                style_class, style, block.content, block.content
            ));
        }

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Converted Book Page</title>
    <style>{}</style>
</head>
<body>
    <div class="page">
        {}
    </div>
</body>
</html>"#,
            style, content
        )
    }
}
