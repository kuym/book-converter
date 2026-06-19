use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};
use std::path::Path;
use std::time::Duration;

/// Wraps a single headless Chrome instance reused across iterations.
pub struct Renderer {
    browser: Browser,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Launch headless Chrome with a window sized to the page being rendered,
    /// so screenshots line up with the source photo's aspect ratio.
    pub fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = LaunchOptions::default_builder();
        builder.window_size(Some((width, height)));
        // Keep Chrome alive across iterations: a refinement LLM call between
        // renders can easily exceed the default 30s idle timeout, which would
        // otherwise shut the browser down and break the next screenshot.
        builder.idle_browser_timeout(Duration::from_secs(3600));

        // Prefer a system-installed Chrome/Chromium/Edge. If none is found we
        // leave `path` unset so the `fetch` feature downloads a known-good
        // Chromium build (cached for subsequent runs).
        match headless_chrome::browser::default_executable() {
            Ok(path) => {
                builder.path(Some(path));
            }
            Err(_) => {
                println!("  no system Chrome found; downloading a Chromium build (first run only)...");
            }
        }

        let options = builder
            .build()
            .map_err(|e| format!("failed to configure headless Chrome: {}", e))?;

        let browser = Browser::new(options).map_err(|e| {
            format!(
                "failed to launch headless Chrome (is Google Chrome / Chromium installed?): {}",
                e
            )
        })?;

        Ok(Self {
            browser,
            width,
            height,
        })
    }

    /// Load an HTML file from disk and capture a PNG screenshot of the page.
    pub fn screenshot(&self, html_path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let abs = html_path.canonicalize()?;
        let url = format!("file://{}", abs.to_string_lossy());

        let tab = self.browser.new_tab()?;
        tab.set_default_timeout(Duration::from_secs(30));
        tab.navigate_to(&url)?;
        tab.wait_until_navigated()?;

        let png = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)?;
        Ok(png)
    }

    pub fn window_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
