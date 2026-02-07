//! Notification types for the notification system.

use serde::{Deserialize, Serialize};

/// Notification type categories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Success,
    Error,
    Warning,
    Info,
}

impl NotificationType {
    pub fn as_str(&self) -> &str {
        match self {
            NotificationType::Success => "success",
            NotificationType::Error => "error",
            NotificationType::Warning => "warning",
            NotificationType::Info => "info",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "success" => NotificationType::Success,
            "error" => NotificationType::Error,
            "warning" => NotificationType::Warning,
            "info" => NotificationType::Info,
            _ => NotificationType::Info,
        }
    }
}

/// A notification record stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Unique notification ID
    pub id: String,
    /// User ID (profile ID) this notification belongs to
    pub user_id: String,
    /// Notification type (success, error, warning, info)
    #[serde(rename = "type")]
    pub notification_type: NotificationType,
    /// Short title
    pub title: String,
    /// Optional longer message
    pub message: Option<String>,
    /// Whether the notification has been read
    pub read: bool,
    /// Timestamp in milliseconds
    pub timestamp: i64,
}

/// Paginated notification list response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationListResponse {
    /// Notifications for the current page
    pub notifications: Vec<Notification>,
    /// Total notification count
    pub total: i64,
    /// Current page (1-indexed)
    pub page: i64,
    /// Page size
    pub page_size: i64,
    /// Total number of pages
    pub total_pages: i64,
    /// Number of unread notifications
    pub unread_count: i64,
}

/// Query parameters for listing notifications.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationQuery {
    /// Page number (1-indexed, default 1)
    pub page: Option<i64>,
    /// Page size (default 20, max 100)
    pub page_size: Option<i64>,
    /// Only return unread notifications
    pub unread_only: Option<bool>,
}

/// Request to mark specific notifications as read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkReadRequest {
    /// Notification IDs to mark as read. If empty, marks all as read.
    pub ids: Option<Vec<String>>,
}
