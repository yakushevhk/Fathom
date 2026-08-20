use crate::{error, AppState};
use axum::{extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreCredentialRequest { pub name: String, pub kind: String, pub secret: String }

#[derive(Debug, Serialize)]
pub struct CredentialResponse { pub id: String, pub name: String, pub kind: String, pub created_at: String, pub updated_at: String }

impl From<pr_persistence::CredentialRow> for CredentialResponse {
    fn from(row: pr_persistence::CredentialRow) -> Self { Self { id: row.id, name: row.name, kind: row.kind, created_at: row.created_at, updated_at: row.updated_at } }
}

pub async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_credentials() { Ok(rows) => Json(rows.into_iter().map(CredentialResponse::from).collect::<Vec<_>>()).into_response(), Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }
}

pub async fn store(State(state): State<Arc<AppState>>, Json(body): Json<StoreCredentialRequest>) -> Response {
    match state.db.store_credential(&body.name, &body.kind, &body.secret) { Ok(row) => (StatusCode::CREATED, Json(CredentialResponse::from(row))).into_response(), Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()) }
}

pub async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.db.delete_credential(&id) { Ok(true) => StatusCode::NO_CONTENT.into_response(), Ok(false) => error(StatusCode::NOT_FOUND, "credential not found"), Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()) }
}
