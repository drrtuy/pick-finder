use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct VisionFeatures {
    pub country: Option<String>,
    pub denomination: Option<String>,
    pub currency: Option<String>,
    pub year: Option<u16>,
    pub portrait: Option<String>,
    pub signatures: Option<Vec<String>>,
}

/// Extract features from a banknote image using OpenAI gpt-4o vision.
pub fn extract_features(image_path: &Path, openai_key: &str) -> Option<VisionFeatures> {
    let image_bytes = std::fs::read(image_path).ok()?;
    let b64 = BASE64.encode(&image_bytes);
    let data_url = format!("data:image/jpeg;base64,{b64}");

    let client = reqwest::blocking::Client::new();

    let body = serde_json::json!({
        "model": "gpt-4o",
        "max_tokens": 300,
        "messages": [
            {
                "role": "system",
                "content": "You are a numismatic expert. Analyze banknote images and extract metadata. Always respond with a single JSON object, no markdown fences."
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Extract the following from this banknote image and return as JSON:\n{\"country\": \"...\", \"denomination\": \"...\", \"currency\": \"...\", \"year\": NNNN, \"portrait\": \"name of person depicted or null\", \"signatures\": [\"name1\", ...] or null}\nUse numeric denomination (e.g. \"20\" not \"twenty\"). Year must be an integer. Return only the JSON."
                    },
                    {
                        "type": "image_url",
                        "image_url": { "url": data_url, "detail": "low" }
                    }
                ]
            }
        ]
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {openai_key}"))
        .json(&body)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        eprintln!("  [vision] OpenAI API error {status}: {text}");
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    let content = json["choices"][0]["message"]["content"].as_str()?;

    // Strip markdown fences if present
    let clean = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(clean).ok()
}
