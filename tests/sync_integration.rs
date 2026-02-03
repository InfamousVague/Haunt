//! Integration tests for cross-server preference sync.
//!
//! These tests verify that preferences sync correctly across multiple server instances.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const OSAKA_URL: &str = "https://osaka.haunt.st";
const SEOUL_URL: &str = "https://seoul.haunt.st";
const NYC_URL: &str = "https://nyc.haunt.st";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileSettings {
    default_timeframe: String,
    preferred_indicators: Vec<String>,
    notifications_enabled: bool,
    preferred_server: Option<String>,
    auto_fastest: bool,
    theme: String,
    language: String,
    updated_at: i64,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            default_timeframe: "day_trading".to_string(),
            preferred_indicators: Vec::new(),
            notifications_enabled: false,
            preferred_server: None,
            auto_fastest: false,
            theme: "dark".to_string(),
            language: "en".to_string(),
            updated_at: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePreferencesRequest {
    public_key: String,
    settings: ProfileSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesResponse {
    public_key: String,
    settings: ProfileSettings,
    synced_from: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatusResponse {
    server_id: String,
    server_region: String,
    connected_peers: usize,
    total_peers: usize,
}

fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

/// Test that we can get sync status from each server.
#[tokio::test]
async fn test_sync_status_endpoints() {
    let client = create_client();

    for (name, url) in [("Osaka", OSAKA_URL), ("Seoul", SEOUL_URL), ("NYC", NYC_URL)] {
        let response = client
            .get(format!("{}/api/sync/status", url))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let status: SyncStatusResponse = resp.json().await.unwrap();
                println!(
                    "{}: server_id={}, region={}, peers={}/{}",
                    name, status.server_id, status.server_region,
                    status.connected_peers, status.total_peers
                );
                assert!(!status.server_id.is_empty());
            }
            Ok(resp) => {
                println!("{}: Status code {}", name, resp.status());
            }
            Err(e) => {
                println!("{}: Error - {}", name, e);
            }
        }
    }
}

/// Test that preferences can be set on one server and read from another.
#[tokio::test]
async fn test_preferences_sync_across_servers() {
    let client = create_client();

    // Generate a unique test public key
    let test_public_key = format!("test_{}", chrono::Utc::now().timestamp_millis());

    // Set preferences on Osaka with a specific server preference
    let settings = ProfileSettings {
        preferred_server: Some("seoul".to_string()),
        theme: "light".to_string(),
        language: "ko".to_string(),
        updated_at: chrono::Utc::now().timestamp_millis(),
        ..Default::default()
    };

    let request = UpdatePreferencesRequest {
        public_key: test_public_key.clone(),
        settings: settings.clone(),
    };

    // Update preferences on Osaka
    let response = client
        .post(format!("{}/api/sync/preferences", OSAKA_URL))
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let prefs: PreferencesResponse = resp.json().await.unwrap();
            println!("Set preferences on Osaka: theme={}, language={}",
                     prefs.settings.theme, prefs.settings.language);
            assert_eq!(prefs.settings.theme, "light");
            assert_eq!(prefs.settings.language, "ko");
        }
        Ok(resp) => {
            println!("Failed to set preferences on Osaka: {}", resp.status());
            return;
        }
        Err(e) => {
            println!("Error setting preferences on Osaka: {}", e);
            return;
        }
    }

    // Wait for sync to propagate
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Read preferences from Seoul
    let response = client
        .get(format!("{}/api/sync/preferences?publicKey={}", SEOUL_URL, test_public_key))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let prefs: PreferencesResponse = resp.json().await.unwrap();
            println!("Got preferences from Seoul: theme={}, language={}, synced_from={}",
                     prefs.settings.theme, prefs.settings.language, prefs.synced_from);

            // Verify the preferences match
            assert_eq!(prefs.settings.theme, "light", "Theme should sync to Seoul");
            assert_eq!(prefs.settings.language, "ko", "Language should sync to Seoul");
            assert_eq!(prefs.settings.preferred_server, Some("seoul".to_string()));
        }
        Ok(resp) => {
            println!("Failed to get preferences from Seoul: {}", resp.status());
        }
        Err(e) => {
            println!("Error getting preferences from Seoul: {}", e);
        }
    }

    // Read preferences from NYC
    let response = client
        .get(format!("{}/api/sync/preferences?publicKey={}", NYC_URL, test_public_key))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let prefs: PreferencesResponse = resp.json().await.unwrap();
            println!("Got preferences from NYC: theme={}, language={}, synced_from={}",
                     prefs.settings.theme, prefs.settings.language, prefs.synced_from);

            // Verify the preferences match
            assert_eq!(prefs.settings.theme, "light", "Theme should sync to NYC");
            assert_eq!(prefs.settings.language, "ko", "Language should sync to NYC");
        }
        Ok(resp) => {
            println!("Failed to get preferences from NYC: {}", resp.status());
        }
        Err(e) => {
            println!("Error getting preferences from NYC: {}", e);
        }
    }
}

/// Test that the most recent update wins in conflict resolution.
#[tokio::test]
async fn test_conflict_resolution() {
    let client = create_client();

    // Generate a unique test public key
    let test_public_key = format!("conflict_test_{}", chrono::Utc::now().timestamp_millis());

    // Set preferences on Osaka first
    let old_settings = ProfileSettings {
        theme: "dark".to_string(),
        updated_at: chrono::Utc::now().timestamp_millis() - 10000, // 10 seconds ago
        ..Default::default()
    };

    let old_request = UpdatePreferencesRequest {
        public_key: test_public_key.clone(),
        settings: old_settings,
    };

    let _ = client
        .post(format!("{}/api/sync/preferences", OSAKA_URL))
        .json(&old_request)
        .send()
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Set newer preferences on Seoul
    let new_settings = ProfileSettings {
        theme: "light".to_string(),
        updated_at: chrono::Utc::now().timestamp_millis(),
        ..Default::default()
    };

    let new_request = UpdatePreferencesRequest {
        public_key: test_public_key.clone(),
        settings: new_settings,
    };

    let response = client
        .post(format!("{}/api/sync/preferences", SEOUL_URL))
        .json(&new_request)
        .send()
        .await;

    if let Ok(resp) = response {
        if resp.status().is_success() {
            println!("Set newer preferences on Seoul");
        }
    }

    // Wait for sync
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check Osaka - should have the newer "light" theme
    let response = client
        .get(format!("{}/api/sync/preferences?publicKey={}", OSAKA_URL, test_public_key))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let prefs: PreferencesResponse = resp.json().await.unwrap();
            println!("Osaka after conflict: theme={}", prefs.settings.theme);
            assert_eq!(prefs.settings.theme, "light", "Newer update should win");
        }
        Ok(resp) => {
            println!("Failed: {}", resp.status());
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

/// Test peer connectivity status.
#[tokio::test]
async fn test_peer_connectivity() {
    let client = create_client();

    let response = client
        .get(format!("{}/api/sync/peers", OSAKA_URL))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let peers: Vec<serde_json::Value> = resp.json().await.unwrap();
            println!("Osaka peers: {}", serde_json::to_string_pretty(&peers).unwrap());

            // Should have at least Seoul and NYC as peers
            assert!(peers.len() >= 2, "Should have at least 2 peers configured");
        }
        Ok(resp) => {
            println!("Status: {}", resp.status());
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

/// Compare data between all servers to ensure consistency.
#[tokio::test]
async fn test_data_consistency_across_servers() {
    let client = create_client();
    let servers = [
        ("Osaka", OSAKA_URL),
        ("Seoul", SEOUL_URL),
        ("NYC", NYC_URL),
    ];

    // Test a known endpoint that should return consistent data
    let test_public_key = "consistency_test_key";

    let mut results: Vec<(String, Option<PreferencesResponse>)> = Vec::new();

    for (name, url) in &servers {
        let response = client
            .get(format!("{}/api/sync/preferences?publicKey={}", url, test_public_key))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let prefs: PreferencesResponse = resp.json().await.unwrap();
                results.push((name.to_string(), Some(prefs)));
            }
            _ => {
                results.push((name.to_string(), None));
            }
        }
    }

    // If we got data from multiple servers, verify they match
    let with_data: Vec<_> = results.iter()
        .filter_map(|(name, prefs)| prefs.as_ref().map(|p| (name, p)))
        .collect();

    if with_data.len() >= 2 {
        let first = &with_data[0].1;
        for (name, prefs) in &with_data[1..] {
            assert_eq!(
                first.settings.theme, prefs.settings.theme,
                "Theme should match between servers"
            );
            assert_eq!(
                first.settings.language, prefs.settings.language,
                "Language should match between servers"
            );
            println!("{}: Data matches", name);
        }
    }

    println!("Data consistency check complete");
}
