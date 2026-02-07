//! Notifications API
//!
//! Endpoints for managing user notifications:
//!
//! - GET    /api/notifications           - List notifications (paginated)
//! - POST   /api/notifications/read      - Mark notifications as read
//! - POST   /api/notifications/read-all  - Mark all notifications as read
//! - DELETE /api/notifications           - Clear all notification history

use axum::{
    extract::{Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;

use crate::api::auth::Authenticated;
use crate::types::{NotificationListResponse, NotificationQuery, MarkReadRequest};
use crate::AppState;

// =============================================================================
// Router
// =============================================================================

/// Create notifications router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notifications))
        .route("/read", post(mark_read))
        .route("/read-all", post(mark_all_read))
        .route("/", delete(clear_notifications))
}

// =============================================================================
// Response types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub affected: i64,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/notifications
///
/// List notifications for the authenticated user with pagination.
async fn list_notifications(
    auth: Authenticated,
    State(state): State<AppState>,
    Query(query): Query<NotificationQuery>,
) -> Json<ApiResponse<NotificationListResponse>> {
    let user_id = &auth.user.profile.id;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).min(100).max(1);
    let unread_only = query.unread_only.unwrap_or(false);

    let (notifications, total) = state
        .sqlite_store
        .get_notifications(user_id, page, page_size, unread_only);

    let unread_count = state.sqlite_store.get_unread_notification_count(user_id);

    let total_pages = ((total as f64) / (page_size as f64)).ceil() as i64;

    Json(ApiResponse {
        data: NotificationListResponse {
            notifications,
            total,
            page,
            page_size,
            total_pages: total_pages.max(1),
            unread_count,
        },
    })
}

/// POST /api/notifications/read
///
/// Mark specific notifications as read. If `ids` is empty or omitted, does nothing
/// (use /read-all instead to mark all).
async fn mark_read(
    auth: Authenticated,
    State(state): State<AppState>,
    Json(request): Json<MarkReadRequest>,
) -> Json<ApiResponse<SuccessResponse>> {
    let user_id = &auth.user.profile.id;

    let affected = if let Some(ids) = &request.ids {
        if ids.is_empty() {
            0
        } else {
            state.sqlite_store.mark_notifications_read(user_id, ids)
        }
    } else {
        0
    };

    Json(ApiResponse {
        data: SuccessResponse {
            success: true,
            affected,
        },
    })
}

/// POST /api/notifications/read-all
///
/// Mark all notifications as read for the authenticated user.
async fn mark_all_read(
    auth: Authenticated,
    State(state): State<AppState>,
) -> Json<ApiResponse<SuccessResponse>> {
    let user_id = &auth.user.profile.id;
    let affected = state.sqlite_store.mark_all_notifications_read(user_id);

    Json(ApiResponse {
        data: SuccessResponse {
            success: true,
            affected,
        },
    })
}

/// DELETE /api/notifications
///
/// Clear all notification history for the authenticated user.
async fn clear_notifications(
    auth: Authenticated,
    State(state): State<AppState>,
) -> Json<ApiResponse<SuccessResponse>> {
    let user_id = &auth.user.profile.id;
    let affected = state.sqlite_store.clear_notifications(user_id);

    Json(ApiResponse {
        data: SuccessResponse {
            success: true,
            affected,
        },
    })
}
