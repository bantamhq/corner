use reqwest::Client;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use crate::config::get_config_dir;

static HTTP_CLIENT: LazyLock<Option<Client>> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("Corner/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()
});

pub async fn fetch_calendar(url: &str) -> Result<String, String> {
    // Handle file:// URLs for local ICS files
    if let Some(file_path) = url.strip_prefix("file://") {
        return fetch_local_file(file_path);
    }

    let client = HTTP_CLIENT
        .as_ref()
        .ok_or_else(|| "HTTP client unavailable (TLS initialization failed)".to_string())?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch calendar: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Calendar fetch failed with status: {}",
            response.status()
        ));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read calendar response: {}", e))
}

/// Fetch a local ICS file.
/// Paths can be:
/// - Absolute: /path/to/file.ics
/// - Relative: resolved from config directory (profile or ~/.config/corner)
fn fetch_local_file(path: &str) -> Result<String, String> {
    let path = Path::new(path);
    let file_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        get_config_dir().join(path)
    };

    std::fs::read_to_string(&file_path).map_err(|e| {
        format!(
            "Failed to read local calendar file '{}': {}",
            file_path.display(),
            e
        )
    })
}
