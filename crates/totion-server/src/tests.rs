use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use chrono::Utc;
use http_body_util::BodyExt;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::{api, auth, db};

fn setup() -> (api::Db, String) {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();
    conn.execute(
        "INSERT INTO users(id, name, public_key, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            "user-1",
            "Guan",
            hex::encode([7u8; 32]),
            Utc::now().to_rfc3339()
        ],
    )
    .unwrap();
    let token = auth::create_session(&conn, "user-1").unwrap();
    (Arc::new(Mutex::new(conn)), token)
}

async fn call(
    db: api::Db,
    token: Option<&str>,
    protocol: Option<&str>,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = api::router(db);
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if let Some(protocol) = protocol {
        builder = builder.header("x-totion-protocol", protocol);
    }
    let request = match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[test]
fn migrates_v1_database_in_place() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(db::SCHEMA).unwrap();
    conn.execute(
        "INSERT INTO pages(id, title, icon, kind, position, version, updated_at, updated_by, deleted)
         VALUES ('p1', 'old page', 'P', 'doc', 0, 1, '2026-01-01T00:00:00Z', 'Guan', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO blocks(id, page_id, kind, text, checked, due_date, streak, position, version,
                            updated_at, updated_by, deleted)
         VALUES ('b1', 'p1', 'text', 'old block', 0, NULL, 0, 0, 1, '2026-01-01T00:00:00Z', 'Guan', 0)",
        [],
    )
    .unwrap();

    db::init(&conn).unwrap();
    db::init(&conn).unwrap();

    assert_eq!(
        db::get_meta(&conn, "schema_version").unwrap().as_deref(),
        Some("4")
    );
    let pages: i64 = conn
        .query_row("SELECT COUNT(*) FROM pages", [], |row| row.get(0))
        .unwrap();
    let blocks: i64 = conn
        .query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))
        .unwrap();
    let journals: i64 = conn
        .query_row("SELECT COUNT(*) FROM journals", [], |row| row.get(0))
        .unwrap();
    let invites: i64 = conn
        .query_row("SELECT COUNT(*) FROM invites", [], |row| row.get(0))
        .unwrap();
    assert_eq!((pages, blocks, journals, invites), (1, 1, 0, 0));

    let page = db::find_page(&conn, "p1").unwrap().unwrap();
    assert_eq!(page.title, "old page");
    assert!(page.date.is_none());
    let block = db::find_block(&conn, "b1").unwrap().unwrap();
    assert_eq!(block.text, "old block");
    assert!(block.start_time.is_none());
}

#[tokio::test]
async fn sync_uses_cursor_and_does_not_lose_changes() {
    let (db, token) = setup();
    {
        let conn = db.lock().unwrap();
        for index in 0..1200 {
            db::push_change(
                &conn,
                "page",
                &json!({ "index": index }),
                "Guan",
                Utc::now(),
            )
            .unwrap();
        }
    }

    let mut since = 0;
    let mut total = 0;
    let mut rounds = 0;
    loop {
        let (status, body) = call(
            db.clone(),
            Some(&token),
            Some("2"),
            Method::GET,
            &format!("/api/sync?since={since}&limit=500"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let changes = body["changes"].as_array().unwrap();
        total += changes.len();
        since = body["last_seq"].as_i64().unwrap();
        rounds += 1;
        if !body["has_more"].as_bool().unwrap() {
            break;
        }
        assert!(rounds < 10, "分页没有终止");
    }
    assert_eq!(total, 1200);
    assert_eq!(since, 1200);
}

#[tokio::test]
async fn v1_clients_only_receive_page_and_block_changes() {
    let (db, token) = setup();
    let (status, _) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/habits",
        Some(json!({
            "title": "阅读",
            "icon": "",
            "note": "",
            "frequency": {"type": "daily"},
            "position": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::GET,
        "/api/sync?since=0",
        None,
    )
    .await;
    assert_eq!(body["changes"].as_array().unwrap().len(), 1);
    assert_eq!(body["changes"][0]["entity"], "habit");

    let (_, body) = call(
        db.clone(),
        Some(&token),
        None,
        Method::GET,
        "/api/sync?since=0",
        None,
    )
    .await;
    assert_eq!(body["changes"].as_array().unwrap().len(), 0);
    assert_eq!(body["last_seq"], 0);
}

#[tokio::test]
async fn ensure_day_is_idempotent_and_revives_soft_deleted_page() {
    let (db, token) = setup();
    let (status, first) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/days/2026-11-16/ensure",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["created"].as_bool().unwrap());
    let page_id = first["page"]["id"].as_str().unwrap().to_string();
    assert_eq!(first["blocks"].as_array().unwrap().len(), 3);

    let (_, second) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/days/2026-11-16/ensure",
        None,
    )
    .await;
    assert!(!second["created"].as_bool().unwrap());
    assert_eq!(second["page"]["id"], page_id);
    assert_eq!(second["blocks"].as_array().unwrap().len(), 3);

    let (_, snapshot) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::GET,
        "/api/snapshot",
        None,
    )
    .await;
    assert_eq!(snapshot["pages"].as_array().unwrap().len(), 1);

    let (status, _) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/pages/{page_id}"),
        Some(json!({ "expected_version": 1, "deleted": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, revived) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/days/2026-11-16/ensure",
        None,
    )
    .await;
    assert!(!revived["page"]["deleted"].as_bool().unwrap());
    assert_eq!(revived["page"]["id"], page_id);
}

#[tokio::test]
async fn habit_checkin_is_idempotent_and_can_be_undone() {
    let (db, token) = setup();
    let (status, habit) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/habits",
        Some(json!({
            "title": "跑步",
            "icon": "",
            "note": "",
            "frequency": {"type": "weekdays"},
            "position": 0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let habit_id = habit["habit"]["id"].as_str().unwrap().to_string();

    let (status, first) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        &format!("/api/habits/{habit_id}/checkins"),
        Some(json!({ "date": "2026-11-16", "note": "5km" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["created"].as_bool().unwrap());
    assert!(first["seq"].is_number());

    let (status, second) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        &format!("/api/habits/{habit_id}/checkins"),
        Some(json!({ "date": "2026-11-16" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!second["created"].as_bool().unwrap());
    assert!(second["seq"].is_null());

    let checkin_id = first["checkin"]["id"].as_str().unwrap().to_string();
    let (status, _) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/checkins/{checkin_id}"),
        Some(json!({ "expected_version": 1, "deleted": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, third) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        &format!("/api/habits/{habit_id}/checkins"),
        Some(json!({ "date": "2026-11-16" })),
    )
    .await;
    assert!(third["created"].as_bool().unwrap());
}

#[tokio::test]
async fn comments_validate_target_and_conflict() {
    let (db, token) = setup();
    let (status, _) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/comments",
        Some(json!({
            "target_kind": "page",
            "target_id": "missing",
            "body": "hello"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, page) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/pages",
        Some(json!({
            "title": "计划",
            "icon": "P",
            "kind": "plan",
            "position": 0
        })),
    )
    .await;
    let page_id = page["page"]["id"].as_str().unwrap().to_string();

    let (status, comment) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/comments",
        Some(json!({
            "target_kind": "page",
            "target_id": page_id,
            "body": "这条计划不错"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comment["comment"]["author_name"], "Guan");
    let comment_id = comment["comment"]["id"].as_str().unwrap().to_string();

    let (status, body) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/comments/{comment_id}"),
        Some(json!({ "expected_version": 1, "resolved": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["comment"]["resolved"].as_bool().unwrap());

    let (status, body) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/comments/{comment_id}"),
        Some(json!({ "expected_version": 1, "resolved": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["current"]["version"], 2);
}

#[tokio::test]
async fn multiple_invites_join_same_space_and_can_be_revoked() {
    let (db, token) = setup();

    let (status, first) = call(
        db.clone(),
        Some(&token),
        Some("4"),
        Method::POST,
        "/api/invites",
        Some(json!({ "note": "once", "max_uses": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let once_code = first["invite"]["code"].as_str().unwrap().to_string();

    let (_, second) = call(
        db.clone(),
        Some(&token),
        Some("4"),
        Method::POST,
        "/api/invites",
        Some(json!({ "note": "open" })),
    )
    .await;
    let open_code = second["invite"]["code"].as_str().unwrap().to_string();
    let open_id = second["invite"]["id"].as_str().unwrap().to_string();

    let (status, _) = call(
        db.clone(),
        None,
        Some("4"),
        Method::POST,
        "/api/auth/register",
        Some(json!({
            "invite_code": once_code,
            "name": "Bob",
            "public_key": hex::encode([2u8; 32])
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        db.clone(),
        None,
        Some("4"),
        Method::POST,
        "/api/auth/register",
        Some(json!({
            "invite_code": once_code,
            "name": "Carol",
            "public_key": hex::encode([3u8; 32])
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(
        db.clone(),
        None,
        Some("4"),
        Method::POST,
        "/api/auth/register",
        Some(json!({
            "invite_code": open_code,
            "name": "Dave",
            "public_key": hex::encode([4u8; 32])
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        db.clone(),
        Some(&token),
        Some("4"),
        Method::PATCH,
        &format!("/api/invites/{open_id}"),
        Some(json!({ "revoked": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        db.clone(),
        None,
        Some("4"),
        Method::POST,
        "/api/auth/register",
        Some(json!({
            "invite_code": open_code,
            "name": "Eve",
            "public_key": hex::encode([5u8; 32])
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_, snapshot) = call(
        db.clone(),
        Some(&token),
        Some("4"),
        Method::GET,
        "/api/snapshot",
        None,
    )
    .await;
    assert_eq!(snapshot["workspace"]["users"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn block_completion_records_timestamp_and_actor() {
    let (db, token) = setup();
    let (_, page) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/pages",
        Some(json!({
            "title": "待办",
            "icon": "T",
            "kind": "todo",
            "position": 0
        })),
    )
    .await;
    let page_id = page["page"]["id"].as_str().unwrap().to_string();

    let (_, block) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::POST,
        "/api/blocks",
        Some(json!({
            "page_id": page_id,
            "kind": "todo",
            "text": "写文档",
            "due_date": "2026-11-16",
            "position": 0,
            "start_time": "09:00"
        })),
    )
    .await;
    let block_id = block["block"]["id"].as_str().unwrap().to_string();
    assert!(block["block"]["completed_at"].is_null());

    let (_, done) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/blocks/{block_id}"),
        Some(json!({ "expected_version": 1, "checked": true })),
    )
    .await;
    assert!(done["block"]["completed_at"].is_string());
    assert_eq!(done["block"]["completed_by"], "Guan");

    let (_, undone) = call(
        db.clone(),
        Some(&token),
        Some("2"),
        Method::PATCH,
        &format!("/api/blocks/{block_id}"),
        Some(json!({ "expected_version": 2, "checked": false })),
    )
    .await;
    assert!(undone["block"]["completed_at"].is_null());
    assert!(undone["block"]["completed_by"].is_null());
}
