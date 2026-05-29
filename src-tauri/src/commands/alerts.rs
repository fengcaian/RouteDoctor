use once_cell::sync::Lazy;
use serde_json::Value;

static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Trigger a webhook (POST JSON to the given URL).
/// Returns the HTTP status code on success, or an error message on failure.
#[tauri::command]
pub async fn trigger_webhook(url: String, payload: Value) -> Result<u16, String> {
    let resp = HTTP
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("webhook request failed: {}", e))?;

    Ok(resp.status().as_u16())
}
