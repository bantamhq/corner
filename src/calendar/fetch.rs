use reqwest::Client;
use std::sync::LazyLock;
use std::time::Duration;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Caliber/1.0")
        .build()
        .expect("Failed to create HTTP client")
});

pub async fn fetch_calendar(url: &str) -> Result<String, String> {
    let response = HTTP_CLIENT
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
