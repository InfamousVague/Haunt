//! Username Validation and Content Moderation Service
//!
//! Provides comprehensive username validation including:
//! - Length and format validation
//! - Profanity and inappropriate content filtering
//! - Homoglyph/lookalike character detection
//! - Reserved username protection
//! - Rate limiting for username changes

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Username validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    /// Whether the username is valid.
    pub is_valid: bool,
    /// Error message if invalid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Error code for client handling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<UsernameErrorCode>,
    /// Sanitized/normalized version of the username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    /// Suggestions if username is taken or invalid.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

/// Username validation error codes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UsernameErrorCode {
    TooShort,
    TooLong,
    InvalidCharacters,
    StartsWithNumber,
    ConsecutiveSpecialChars,
    ProfanityDetected,
    ReservedUsername,
    HomoglyphDetected,
    AlreadyTaken,
    RateLimited,
}

/// Username filter configuration.
#[derive(Debug, Clone)]
pub struct UsernameFilterConfig {
    /// Minimum username length.
    pub min_length: usize,
    /// Maximum username length.
    pub max_length: usize,
    /// Allowed special characters.
    pub allowed_special_chars: Vec<char>,
    /// Rate limit window for username changes.
    pub rate_limit_window: Duration,
    /// Maximum username changes per window.
    pub max_changes_per_window: u32,
}

impl Default for UsernameFilterConfig {
    fn default() -> Self {
        Self {
            min_length: 3,
            max_length: 30,
            allowed_special_chars: vec!['_', '-', '.'],
            rate_limit_window: Duration::from_secs(24 * 60 * 60), // 24 hours
            max_changes_per_window: 3,
        }
    }
}

/// Homoglyph mappings (characters that look similar).
fn get_homoglyph_map() -> Vec<(char, char)> {
    vec![
        ('0', 'o'),
        ('0', 'O'),
        ('1', 'l'),
        ('1', 'I'),
        ('1', 'i'),
        ('3', 'e'),
        ('3', 'E'),
        ('4', 'a'),
        ('4', 'A'),
        ('5', 's'),
        ('5', 'S'),
        ('6', 'b'),
        ('6', 'G'),
        ('7', 't'),
        ('7', 'T'),
        ('8', 'B'),
        ('9', 'g'),
        ('9', 'q'),
        ('@', 'a'),
        ('$', 's'),
        ('!', 'i'),
        ('!', 'l'),
        ('|', 'l'),
        ('|', 'I'),
        ('(', 'c'),
        (')', 'd'),
        // Cyrillic lookalikes
        ('а', 'a'), // Cyrillic а
        ('е', 'e'), // Cyrillic е
        ('о', 'o'), // Cyrillic о
        ('р', 'p'), // Cyrillic р
        ('с', 'c'), // Cyrillic с
        ('х', 'x'), // Cyrillic х
        ('у', 'y'), // Cyrillic у
    ]
}

/// Get profanity word list (basic set - extend as needed).
fn get_profanity_list() -> HashSet<String> {
    // Note: This is a minimal example list. In production, use a comprehensive
    // profanity database like badwords, profanity-check, or similar.
    let words = vec![
        // Common profanity (censored examples)
        "admin",
        "moderator",
        "support",
        "helpdesk",
        "official",
        "staff",
        "system",
        "root",
        "null",
        "undefined",
        // Add actual profanity words in production
    ];

    words.into_iter().map(|s| s.to_lowercase()).collect()
}

/// Get reserved usernames.
fn get_reserved_usernames() -> HashSet<String> {
    let reserved = vec![
        // System accounts
        "admin",
        "administrator",
        "root",
        "system",
        "bot",
        "api",
        "support",
        "help",
        "info",
        "contact",
        "abuse",
        "security",
        "moderator",
        "mod",
        "official",
        "staff",
        "team",
        // Brand protection
        "wraith",
        "haunt",
        "ghost",
        "phantom",
        // Common reserved
        "null",
        "undefined",
        "none",
        "void",
        "test",
        "demo",
        "example",
        "sample",
        "guest",
        "anonymous",
        "anon",
        // Service accounts
        "noreply",
        "no-reply",
        "mailer",
        "postmaster",
        "webmaster",
        "hostmaster",
        "ftp",
        "ssh",
        "www",
        "mail",
        "smtp",
        "pop",
        "imap",
    ];

    reserved.into_iter().map(|s| s.to_lowercase()).collect()
}

/// Username change rate limiter entry.
#[derive(Debug, Clone)]
struct RateLimitEntry {
    changes: u32,
    window_start: Instant,
}

/// Username filter service.
pub struct UsernameFilterService {
    config: UsernameFilterConfig,
    profanity_list: HashSet<String>,
    reserved_usernames: HashSet<String>,
    homoglyph_map: Vec<(char, char)>,
    /// Rate limiter: user_id -> rate limit entry
    rate_limiter: DashMap<String, RateLimitEntry>,
    /// Taken usernames cache (normalized form)
    taken_usernames: DashMap<String, String>, // normalized -> original
}

impl UsernameFilterService {
    /// Create a new username filter service.
    pub fn new(config: UsernameFilterConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            profanity_list: get_profanity_list(),
            reserved_usernames: get_reserved_usernames(),
            homoglyph_map: get_homoglyph_map(),
            rate_limiter: DashMap::new(),
            taken_usernames: DashMap::new(),
        })
    }

    /// Create with default configuration.
    pub fn default_service() -> Arc<Self> {
        Self::new(UsernameFilterConfig::default())
    }

    /// Validate a username.
    pub fn validate(&self, username: &str) -> ValidationResult {
        // Normalize for comparison
        let normalized = self.normalize(username);

        // Length check
        if username.len() < self.config.min_length {
            let suggestions = self.generate_suggestions(&normalized);
            return ValidationResult {
                is_valid: false,
                error: Some(format!(
                    "Username must be at least {} characters",
                    self.config.min_length
                )),
                error_code: Some(UsernameErrorCode::TooShort),
                normalized: Some(normalized),
                suggestions,
            };
        }

        if username.len() > self.config.max_length {
            let truncated = &normalized[..self.config.max_length.min(normalized.len())];
            let suggestions = self.generate_suggestions(truncated);
            return ValidationResult {
                is_valid: false,
                error: Some(format!(
                    "Username must be at most {} characters",
                    self.config.max_length
                )),
                error_code: Some(UsernameErrorCode::TooLong),
                normalized: Some(normalized),
                suggestions,
            };
        }

        // Character validation
        if let Some(error) = self.check_characters(username) {
            let suggestions = self.generate_suggestions(&normalized);
            return ValidationResult {
                is_valid: false,
                error: Some(error.0),
                error_code: Some(error.1),
                normalized: Some(normalized),
                suggestions,
            };
        }

        // Reserved username check
        if self.reserved_usernames.contains(&normalized) {
            let suggestions = self.generate_suggestions(&normalized);
            return ValidationResult {
                is_valid: false,
                error: Some("This username is reserved".to_string()),
                error_code: Some(UsernameErrorCode::ReservedUsername),
                normalized: Some(normalized),
                suggestions,
            };
        }

        // Profanity check
        if self.contains_profanity(&normalized) {
            return ValidationResult {
                is_valid: false,
                error: Some("Username contains inappropriate content".to_string()),
                error_code: Some(UsernameErrorCode::ProfanityDetected),
                normalized: Some(normalized),
                suggestions: vec![],
            };
        }

        // Homoglyph attack detection
        if self.detect_homoglyph_attack(username) {
            let suggestions = self.generate_suggestions(&normalized);
            return ValidationResult {
                is_valid: false,
                error: Some("Username contains confusing characters".to_string()),
                error_code: Some(UsernameErrorCode::HomoglyphDetected),
                normalized: Some(normalized),
                suggestions,
            };
        }

        // Check if taken
        if self.taken_usernames.contains_key(&normalized) {
            let suggestions = self.generate_suggestions(&normalized);
            return ValidationResult {
                is_valid: false,
                error: Some("Username is already taken".to_string()),
                error_code: Some(UsernameErrorCode::AlreadyTaken),
                normalized: Some(normalized),
                suggestions,
            };
        }

        ValidationResult {
            is_valid: true,
            error: None,
            error_code: None,
            normalized: Some(normalized),
            suggestions: vec![],
        }
    }

    /// Check rate limit for username changes.
    pub fn check_rate_limit(&self, user_id: &str) -> bool {
        let now = Instant::now();

        if let Some(mut entry) = self.rate_limiter.get_mut(user_id) {
            // Check if window has expired
            if now.duration_since(entry.window_start) > self.config.rate_limit_window {
                // Reset window
                entry.changes = 1;
                entry.window_start = now;
                return true;
            }

            // Check if limit exceeded
            if entry.changes >= self.config.max_changes_per_window {
                return false;
            }

            entry.changes += 1;
            true
        } else {
            // First change
            self.rate_limiter.insert(
                user_id.to_string(),
                RateLimitEntry {
                    changes: 1,
                    window_start: now,
                },
            );
            true
        }
    }

    /// Validate with rate limit check.
    pub fn validate_with_rate_limit(&self, username: &str, user_id: &str) -> ValidationResult {
        if !self.check_rate_limit(user_id) {
            return ValidationResult {
                is_valid: false,
                error: Some("Too many username changes. Please try again later.".to_string()),
                error_code: Some(UsernameErrorCode::RateLimited),
                normalized: None,
                suggestions: vec![],
            };
        }

        self.validate(username)
    }

    /// Register a taken username.
    pub fn register_username(&self, username: &str) {
        let normalized = self.normalize(username);
        self.taken_usernames
            .insert(normalized, username.to_string());
    }

    /// Unregister a username (when user deletes account or changes username).
    pub fn unregister_username(&self, username: &str) {
        let normalized = self.normalize(username);
        self.taken_usernames.remove(&normalized);
    }

    /// Normalize username for comparison.
    fn normalize(&self, username: &str) -> String {
        let mut normalized = username.to_lowercase();

        // Replace homoglyphs with their base characters
        for (lookalike, base) in &self.homoglyph_map {
            normalized = normalized.replace(*lookalike, &base.to_string());
        }

        // Remove special characters for comparison
        normalized
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }

    /// Check character validity.
    fn check_characters(&self, username: &str) -> Option<(String, UsernameErrorCode)> {
        let chars: Vec<char> = username.chars().collect();

        // Check first character
        if chars.first().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return Some((
                "Username cannot start with a number".to_string(),
                UsernameErrorCode::StartsWithNumber,
            ));
        }

        // Check all characters
        let mut prev_special = false;
        for c in &chars {
            let is_special = self.config.allowed_special_chars.contains(c);

            if !c.is_alphanumeric() && !is_special {
                return Some((
                    format!("Username contains invalid character: {}", c),
                    UsernameErrorCode::InvalidCharacters,
                ));
            }

            // Check consecutive special characters
            if is_special && prev_special {
                return Some((
                    "Username cannot have consecutive special characters".to_string(),
                    UsernameErrorCode::ConsecutiveSpecialChars,
                ));
            }

            prev_special = is_special;
        }

        None
    }

    /// Check if username contains profanity.
    fn contains_profanity(&self, normalized: &str) -> bool {
        // Check exact match
        if self.profanity_list.contains(normalized) {
            return true;
        }

        // Check if contains any profanity word
        for word in &self.profanity_list {
            if word.len() >= 4 && normalized.contains(word) {
                return true;
            }
        }

        false
    }

    /// Detect potential homoglyph attacks (mixing character sets).
    fn detect_homoglyph_attack(&self, username: &str) -> bool {
        let mut has_ascii = false;
        let mut has_non_ascii = false;

        for c in username.chars() {
            if c.is_ascii_alphanumeric() {
                has_ascii = true;
            } else if c.is_alphanumeric() {
                has_non_ascii = true;
            }

            // Mixed scripts are suspicious
            if has_ascii && has_non_ascii {
                return true;
            }
        }

        false
    }

    /// Generate username suggestions.
    fn generate_suggestions(&self, base: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let suffixes = ["_", "1", "2", "x", "_x", "99", "00"];

        for suffix in suffixes {
            let suggestion = format!("{}{}", base, suffix);
            if suggestion.len() <= self.config.max_length {
                let normalized = self.normalize(&suggestion);
                if !self.taken_usernames.contains_key(&normalized)
                    && !self.reserved_usernames.contains(&normalized)
                {
                    suggestions.push(suggestion);
                    if suggestions.len() >= 3 {
                        break;
                    }
                }
            }
        }

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_username() {
        let service = UsernameFilterService::default_service();

        let result = service.validate("validuser");
        assert!(result.is_valid);

        let result = service.validate("user_name");
        assert!(result.is_valid);

        let result = service.validate("User123");
        assert!(result.is_valid);
    }

    #[test]
    fn test_too_short() {
        let service = UsernameFilterService::default_service();

        let result = service.validate("ab");
        assert!(!result.is_valid);
        assert_eq!(result.error_code, Some(UsernameErrorCode::TooShort));
    }

    #[test]
    fn test_too_long() {
        let service = UsernameFilterService::default_service();

        let result = service.validate("thisusernameiswaytoolongtobevalid");
        assert!(!result.is_valid);
        assert_eq!(result.error_code, Some(UsernameErrorCode::TooLong));
    }

    #[test]
    fn test_starts_with_number() {
        let service = UsernameFilterService::default_service();

        let result = service.validate("123user");
        assert!(!result.is_valid);
        assert_eq!(result.error_code, Some(UsernameErrorCode::StartsWithNumber));
    }

    #[test]
    fn test_reserved_username() {
        let service = UsernameFilterService::default_service();

        let result = service.validate("admin");
        assert!(!result.is_valid);
        assert_eq!(result.error_code, Some(UsernameErrorCode::ReservedUsername));
    }

    #[test]
    fn test_taken_username() {
        let service = UsernameFilterService::default_service();

        service.register_username("takenuser");

        let result = service.validate("takenuser");
        assert!(!result.is_valid);
        assert_eq!(result.error_code, Some(UsernameErrorCode::AlreadyTaken));

        // Also check normalized version
        let result = service.validate("TakenUser");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_rate_limiting() {
        let config = UsernameFilterConfig {
            max_changes_per_window: 2,
            ..Default::default()
        };
        let service = UsernameFilterService::new(config);

        assert!(service.check_rate_limit("user1"));
        assert!(service.check_rate_limit("user1"));
        assert!(!service.check_rate_limit("user1")); // Should fail on 3rd attempt
    }
}
