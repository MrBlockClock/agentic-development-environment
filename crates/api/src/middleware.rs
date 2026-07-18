use axum::{extract::Request, middleware::Next, response::Response};

pub async fn auth_middleware(req: Request, next: Next) -> Response {
    // TODO: validate session token
    next.run(req).await
}

pub async fn audit_middleware(req: Request, next: Next) -> Response {
    // TODO: log audit entry for state-changing requests
    next.run(req).await
}
