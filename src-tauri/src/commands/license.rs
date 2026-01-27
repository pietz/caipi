use crate::storage;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// Lemon Squeezy store and product IDs for validation
const EXPECTED_STORE_ID: u64 = 280713;
const EXPECTED_PRODUCT_ID: u64 = 795372;

const LEMON_SQUEEZY_ACTIVATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/activate";
const LEMON_SQUEEZY_VALIDATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/validate";
const LEMON_SQUEEZY_DEACTIVATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/deactivate";

/// License status returned to the frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseStatus {
    pub valid: bool,
    pub license_key: Option<String>,
    pub activated_at: Option<u64>,
    pub email: Option<String>,
}

/// Result of a license validation attempt
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub email: Option<String>,
}

// Lemon Squeezy API response structures
#[derive(Debug, Deserialize)]
struct LemonSqueezyActivateResponse {
    activated: bool,
    instance: Option<LemonSqueezyInstance>,
    error: Option<String>,
    license_key: Option<LemonSqueezyLicenseKey>,
    meta: Option<LemonSqueezyMeta>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LemonSqueezyValidateResponse {
    valid: bool,
    error: Option<String>,
    license_key: Option<LemonSqueezyLicenseKey>,
    meta: Option<LemonSqueezyMeta>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LemonSqueezyDeactivateResponse {
    deactivated: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyInstance {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyLicenseKey {
    status: String,
}

#[derive(Debug, Deserialize)]
struct LemonSqueezyMeta {
    store_id: u64,
    product_id: u64,
    customer_email: Option<String>,
}

/// Get a unique instance name for this machine
fn get_instance_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| format!("caipi-{}", uuid::Uuid::new_v4()))
}

/// Validate and activate a license key with Lemon Squeezy
#[tauri::command]
pub async fn validate_license(license_key: String) -> Result<LicenseValidationResult, String> {
    let trimmed_key = license_key.trim().to_string();

    if trimmed_key.is_empty() {
        return Ok(LicenseValidationResult {
            valid: false,
            error: Some("License key cannot be empty".to_string()),
            email: None,
        });
    }

    let client = reqwest::Client::new();
    let instance_name = get_instance_name();

    // Call Lemon Squeezy activate endpoint
    let response = client
        .post(LEMON_SQUEEZY_ACTIVATE_URL)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("license_key", trimmed_key.as_str()),
            ("instance_name", instance_name.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to connect to license server: {}", e))?;

    let status = response.status();
    let response_text = response.text().await.map_err(|e| e.to_string())?;

    // Parse the response
    let activate_response: LemonSqueezyActivateResponse =
        serde_json::from_str(&response_text).map_err(|e| {
            format!(
                "Failed to parse license server response: {} (status: {})",
                e, status
            )
        })?;

    // Check for API-level errors
    if let Some(error) = activate_response.error {
        return Ok(LicenseValidationResult {
            valid: false,
            error: Some(error),
            email: None,
        });
    }

    // Verify activation was successful
    if !activate_response.activated {
        return Ok(LicenseValidationResult {
            valid: false,
            error: Some("License activation failed".to_string()),
            email: None,
        });
    }

    // Security: Verify store_id and product_id match our expected values
    // This prevents cross-product license abuse
    if let Some(meta) = &activate_response.meta {
        if meta.store_id != EXPECTED_STORE_ID {
            return Ok(LicenseValidationResult {
                valid: false,
                error: Some("Invalid license: store mismatch".to_string()),
                email: None,
            });
        }
        if meta.product_id != EXPECTED_PRODUCT_ID {
            return Ok(LicenseValidationResult {
                valid: false,
                error: Some("Invalid license: product mismatch".to_string()),
                email: None,
            });
        }
    }

    // Check license key status
    if let Some(license_key_info) = &activate_response.license_key {
        if license_key_info.status != "active" {
            return Ok(LicenseValidationResult {
                valid: false,
                error: Some(format!(
                    "License is not active (status: {})",
                    license_key_info.status
                )),
                email: None,
            });
        }
    }

    // Extract instance_id for future deactivation
    let instance_id = activate_response.instance.map(|i| i.id);

    // Extract email from meta
    let email = activate_response
        .meta
        .as_ref()
        .and_then(|m| m.customer_email.clone());

    // Store the validated license
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    storage::set_license(trimmed_key, now, email.clone(), instance_id)
        .map_err(|e| e.to_string())?;

    Ok(LicenseValidationResult {
        valid: true,
        error: None,
        email,
    })
}

/// Get the current license status
#[tauri::command]
pub async fn get_license_status() -> Result<LicenseStatus, String> {
    let license_data = storage::get_license().map_err(|e| e.to_string())?;

    match license_data {
        Some(data) => Ok(LicenseStatus {
            valid: true,
            license_key: Some(mask_license_key(&data.license_key)),
            activated_at: Some(data.activated_at),
            email: data.email,
        }),
        None => Ok(LicenseStatus {
            valid: false,
            license_key: None,
            activated_at: None,
            email: None,
        }),
    }
}

/// Clear/deactivate the current license
#[tauri::command]
pub async fn clear_license() -> Result<(), String> {
    // Get current license to check for instance_id
    let license_data = storage::get_license().map_err(|e| e.to_string())?;

    // If we have license data with an instance_id, deactivate it on Lemon Squeezy
    if let Some(data) = license_data {
        if let Some(instance_id) = data.instance_id {
            let client = reqwest::Client::new();

            // Attempt to deactivate - we don't fail if this doesn't work
            // (user might be offline, etc.)
            let _ = client
                .post(LEMON_SQUEEZY_DEACTIVATE_URL)
                .header("Accept", "application/json")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .form(&[
                    ("license_key", data.license_key.as_str()),
                    ("instance_id", instance_id.as_str()),
                ])
                .send()
                .await;
        }
    }

    storage::clear_license().map_err(|e| e.to_string())?;
    Ok(())
}

/// Mask a license key for display (show first 10 chars, mask the rest)
fn mask_license_key(key: &str) -> String {
    if key.len() <= 10 {
        return key.to_string();
    }
    let visible = &key[..10];
    let masked_len = key.len() - 10;
    format!("{}...{}", visible, "*".repeat(masked_len.min(8)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_license_key_short() {
        assert_eq!(mask_license_key("SHORT"), "SHORT");
        assert_eq!(mask_license_key("EXACTLY10C"), "EXACTLY10C");
    }

    #[test]
    fn test_mask_license_key_long() {
        let result = mask_license_key("ABCDEFGHIJ1234567890");
        assert!(result.starts_with("ABCDEFGHIJ"));
        assert!(result.contains("..."));
        assert!(result.contains("*"));
    }

    #[test]
    fn test_get_instance_name() {
        let name = get_instance_name();
        assert!(!name.is_empty());
    }
}
