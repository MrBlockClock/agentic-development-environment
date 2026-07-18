use crate::router::ApiState;
use axum::{
    extract::{Request, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub async fn auth_middleware(State(state): State<ApiState>, req: Request, next: Next) -> Response {
    let Some(expected) = state.auth_token() else {
        return next.run(req).await;
    };
    let supplied = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if supplied.is_some_and(|value| constant_time_eq(value.as_bytes(), expected.as_bytes())) {
        return next.run(req).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "code": "unauthorized",
                "message": "valid bearer token required"
            }
        })),
    )
        .into_response()
}

pub async fn audit_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    let started = std::time::Instant::now();
    let mut response = next.run(req).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-ade-request-id"), value);
    }
    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = response.status().as_u16(),
        elapsed_ms = started.elapsed().as_millis(),
        "api request"
    );
    response
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (&left, &right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}
