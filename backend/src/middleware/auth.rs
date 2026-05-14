use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClerkClaims {
    pub sub: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub exp: i64,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub clerk_id: String,
    pub email: String,
    pub name: String,
    pub user_id: Uuid,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let token = req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(json!({"error":"Missing Authorization header"}))))?;

    let claims = verify_clerk_token(&token, &state.config.clerk_jwks_url)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json(json!({"error": e.to_string()}))))?;

    let name = [claims.first_name.as_deref(), claims.last_name.as_deref()]
        .iter().filter_map(|s| *s).collect::<Vec<_>>().join(" ");
    let email = claims.email.unwrap_or_default();

    let user = state.postgres
        .upsert_user(&claims.sub, &email, &name, "researcher")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    req.extensions_mut().insert(AuthUser {
        clerk_id: claims.sub,
        email: user.email,
        name: user.name,
        user_id: user.id,
    });

    Ok(next.run(req).await)
}

async fn verify_clerk_token(token: &str, jwks_url: &str) -> anyhow::Result<ClerkClaims> {
    let client = reqwest::Client::new();
    let jwks: Value = client.get(jwks_url).send().await?.json().await?;

    let header = jsonwebtoken::decode_header(token)?;
    let kid = header.kid.unwrap_or_default();

    let keys = jwks["keys"].as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid JWKS response"))?;

    let jwk = keys.iter()
        .find(|k| k["kid"].as_str().unwrap_or("") == kid)
        .ok_or_else(|| anyhow::anyhow!("No matching JWK found for kid={kid}"))?;

    let n = jwk["n"].as_str().unwrap_or("");
    let e = jwk["e"].as_str().unwrap_or("");

    let decoding_key = jsonwebtoken::DecodingKey::from_rsa_components(n, e)
        .map_err(|e| anyhow::anyhow!("Invalid RSA key: {e}"))?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.validate_exp = true;

    let data = jsonwebtoken::decode::<ClerkClaims>(token, &decoding_key, &validation)
        .map_err(|e| anyhow::anyhow!("JWT invalid: {e}"))?;

    Ok(data.claims)
}
