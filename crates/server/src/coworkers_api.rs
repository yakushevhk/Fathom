//! REST API for desktop-agent coworkers and their channels.
//!
//! Routes are mounted under `/api/v1` by the server router and therefore use
//! the same authentication and rate limiting as the rest of the API.

use crate::{error, json, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

const MAX_ID: usize = 128;
const MAX_NAME: usize = 100;
const MAX_TITLE: usize = 200;
const MAX_ROLE: usize = 100;
const MAX_PROMPT: usize = 20_000;
const MAX_VISIBILITY: usize = 32;

#[derive(Debug, Deserialize)]
pub struct CreateCoworkerRequest {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub role: String,
    pub prompt: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default = "default_active")]
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCoworkerRequest {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub role: String,
    pub prompt: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default = "default_active")]
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub coworker_id: String,
    pub title: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub title: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListChannelsQuery {
    pub coworker_id: Option<String>,
}

fn default_visibility() -> String {
    "private".to_string()
}

fn default_active() -> bool {
    true
}

fn valid_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= MAX_ID
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn bounded(value: &str, max: usize, field: &str, required: bool) -> Result<String, Response> {
    let value = value.trim();
    if required && value.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, format!("{field} must not be empty")));
    }
    if value.chars().count() > max {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("{field} must be at most {max} characters"),
        ));
    }
    Ok(value.to_string())
}

fn coworker_fields(
    name: &str,
    title: &str,
    role: &str,
    prompt: &str,
    visibility: &str,
) -> Result<(String, String, String, String, String), Response> {
    Ok((
        bounded(name, MAX_NAME, "name", true)?,
        bounded(title, MAX_TITLE, "title", false)?,
        bounded(role, MAX_ROLE, "role", false)?,
        bounded(prompt, MAX_PROMPT, "prompt", true)?,
        bounded(visibility, MAX_VISIBILITY, "visibility", true)?,
    ))
}

fn channel_fields(
    coworker_id: &str,
    title: &str,
    session_id: Option<&str>,
) -> Result<(String, String, Option<String>), Response> {
    let coworker_id = bounded(coworker_id, MAX_ID, "coworker_id", true)?;
    if !valid_id(&coworker_id) {
        return Err(error(StatusCode::BAD_REQUEST, "coworker_id is invalid"));
    }
    let title = bounded(title, MAX_TITLE, "title", true)?;
    let session_id = match session_id {
        Some(value) => {
            let value = bounded(value, MAX_ID, "session_id", true)?;
            if !valid_id(&value) {
                return Err(error(StatusCode::BAD_REQUEST, "session_id is invalid"));
            }
            Some(value)
        }
        None => None,
    };
    Ok((coworker_id, title, session_id))
}

fn row_json<T: serde::Serialize>(row: &T, key: &str) -> Response {
    let mut object = serde_json::Map::new();
    object.insert(
        key.to_string(),
        serde_json::to_value(row).unwrap_or(serde_json::Value::Null),
    );
    json(StatusCode::OK, serde_json::Value::Object(object))
}

/// `GET /api/v1/coworkers` — list all configured coworkers.
pub(crate) async fn list_coworkers(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_coworkers() {
        Ok(rows) => json(StatusCode::OK, serde_json::json!({ "coworkers": rows })),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `GET /api/v1/coworkers/:id` — fetch one coworker.
pub(crate) async fn get_coworker(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    if !valid_id(&id) {
        return error(StatusCode::BAD_REQUEST, "coworker id is invalid");
    }
    match state.db.get_coworker(&id) {
        Ok(Some(row)) => row_json(&row, "coworker"),
        Ok(None) => error(StatusCode::NOT_FOUND, "coworker not found"),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `POST /api/v1/coworkers` — create a coworker.
pub(crate) async fn create_coworker(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateCoworkerRequest>,
) -> Response {
    let (name, title, role, prompt, visibility) =
        match coworker_fields(&body.name, &body.title, &body.role, &body.prompt, &body.visibility) {
            Ok(fields) => fields,
            Err(response) => return response,
        };
    match state
        .db
        .create_coworker(&name, &title, &role, &prompt, &visibility, body.active)
    {
        Ok(row) => json(StatusCode::CREATED, serde_json::json!({ "coworker": row })),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `PUT /api/v1/coworkers/:id` — replace a coworker.
pub(crate) async fn update_coworker(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateCoworkerRequest>,
) -> Response {
    if !valid_id(&id) {
        return error(StatusCode::BAD_REQUEST, "coworker id is invalid");
    }
    let (name, title, role, prompt, visibility) =
        match coworker_fields(&body.name, &body.title, &body.role, &body.prompt, &body.visibility) {
            Ok(fields) => fields,
            Err(response) => return response,
        };
    match state.db.update_coworker(&id, &name, &title, &role, &prompt, &visibility, body.active) {
        Ok(Some(row)) => row_json(&row, "coworker"),
        Ok(None) => error(StatusCode::NOT_FOUND, "coworker not found"),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `DELETE /api/v1/coworkers/:id` — delete a coworker.
pub(crate) async fn delete_coworker(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    if !valid_id(&id) {
        return error(StatusCode::BAD_REQUEST, "coworker id is invalid");
    }
    match state.db.delete_coworker(&id) {
        Ok(true) => json(StatusCode::OK, serde_json::json!({ "deleted": true })),
        Ok(false) => error(StatusCode::NOT_FOUND, "coworker not found"),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `GET /api/v1/channels?coworker_id=...` — list channels for a coworker.
pub(crate) async fn list_channels(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListChannelsQuery>,
) -> Response {
    let Some(coworker_id) = query.coworker_id else {
        return error(StatusCode::BAD_REQUEST, "coworker_id is required");
    };
    if !valid_id(&coworker_id) {
        return error(StatusCode::BAD_REQUEST, "coworker_id is invalid");
    }
    match state.db.get_coworker(&coworker_id) {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "coworker not found"),
        Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
    match state.db.list_channels(&coworker_id) {
        Ok(rows) => json(StatusCode::OK, serde_json::json!({ "channels": rows })),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `POST /api/v1/channels` — create a channel, optionally attached to a session.
pub(crate) async fn create_channel(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateChannelRequest>,
) -> Response {
    let (coworker_id, title, session_id) =
        match channel_fields(&body.coworker_id, &body.title, body.session_id.as_deref()) {
            Ok(fields) => fields,
            Err(response) => return response,
        };
    match state.db.get_coworker(&coworker_id) {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "coworker not found"),
        Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
    if let Some(session_id) = session_id.as_deref() {
        match state.db.get_session(&pr_core::SessionId(session_id.to_string())) {
            Ok(Some(_)) => {}
            Ok(None) => return error(StatusCode::NOT_FOUND, "session not found"),
            Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    }
    match state
        .db
        .create_channel(&coworker_id, &title, session_id.as_deref())
    {
        Ok(row) => json(StatusCode::CREATED, serde_json::json!({ "channel": row })),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `PUT /api/v1/channels/:id` — update a channel title/session mapping.
pub(crate) async fn update_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateChannelRequest>,
) -> Response {
    if !valid_id(&id) {
        return error(StatusCode::BAD_REQUEST, "channel id is invalid");
    }
    let title = match bounded(&body.title, MAX_TITLE, "title", true) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let session_id = match body.session_id.as_deref() {
        Some(value) => match bounded(value, MAX_ID, "session_id", true) {
            Ok(value) if valid_id(&value) => Some(value),
            Ok(_) => return error(StatusCode::BAD_REQUEST, "session_id is invalid"),
            Err(response) => return response,
        },
        None => None,
    };
    if let Some(session_id) = session_id.as_deref() {
        match state.db.get_session(&pr_core::SessionId(session_id.to_string())) {
            Ok(Some(_)) => {}
            Ok(None) => return error(StatusCode::NOT_FOUND, "session not found"),
            Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    }
    match state.db.update_channel(&id, &title, session_id.as_deref()) {
        Ok(Some(row)) => row_json(&row, "channel"),
        Ok(None) => error(StatusCode::NOT_FOUND, "channel not found"),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// `DELETE /api/v1/channels/:id` — delete a channel.
pub(crate) async fn delete_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    if !valid_id(&id) {
        return error(StatusCode::BAD_REQUEST, "channel id is invalid");
    }
    match state.db.delete_channel(&id) {
        Ok(true) => json(StatusCode::OK, serde_json::json!({ "deleted": true })),
        Ok(false) => error(StatusCode::NOT_FOUND, "channel not found"),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bounded_fields_and_ids() {
        assert!(coworker_fields("name", "", "", "prompt", "private").is_ok());
        assert!(coworker_fields("", "", "", "prompt", "private").is_err());
        assert!(channel_fields("abc-123", "main", None).is_ok());
        assert!(channel_fields("bad id", "main", None).is_err());
    }
}
