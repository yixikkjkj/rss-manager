//! API regression tests use isolated in-memory databases; they never fetch the Internet.
use crate::{
  auth,
  config::Config,
  db::{self, with_conn},
  state::AppState,
};
use axum::{
  body::{to_bytes, Body},
  http::{Request, StatusCode},
  Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

struct TestApp {
  app: Router,
  state: AppState,
  cookie: String,
}
impl TestApp {
  async fn new() -> Self {
    let config = Arc::new(Config {
      listen_addr: "127.0.0.1:0".into(),
      db_path: ":memory:".into(),
      backup_dir: "".into(),
      log_level: "error".into(),
      fetch_interval: 3600,
      session_secret: "test-secret-000000000000000000000000000".into(),
      initial_password_hash: None,
    });
    let db = db::open(":memory:").await.unwrap();
    with_conn(&db, |c| {
      c.execute("INSERT INTO t_user(id,username,password_hash) VALUES(1,'admin','unused')", [])?;
      Ok(())
    })
    .await
    .unwrap();
    let token = auth::create_session(&db, 1).await.unwrap();
    let cookie = format!("rss_session={}", auth::session_cookie_value(&token, &config.session_secret));
    let scheduler = crate::scheduler::Scheduler::new(db.clone(), config.clone());
    let state = AppState {
      db,
      config,
      scheduler,
      content_lock: Arc::new(tokio::sync::Mutex::new(())),
    };
    Self {
      app: crate::build_router(state.clone()),
      state,
      cookie,
    }
  }
  async fn rpc(&self, name: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
      .method("POST")
      .uri(format!("/api/rpc/{name}"))
      .header("content-type", "application/json")
      .header("cookie", &self.cookie)
      .body(Body::from(body.to_string()))
      .unwrap();
    let response = self.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 10 * 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
  }
  async fn ok(&self, name: &str, body: Value) -> Value {
    let (status, response) = self.rpc(name, body).await;
    assert_eq!(status, StatusCode::OK, "{name}: {response}");
    assert_eq!(response["code"], "0", "{name}: {response}");
    response["data"].clone()
  }
  async fn seed(&self) {
    with_conn(&self.state.db,|c|{
   c.execute("INSERT INTO t_feed(id,user_id,title,xml_url,use_readability) VALUES(1,1,'Fixture','https://fixture.invalid/feed',1)",[])?;
   c.execute("INSERT INTO t_article(id,feed_id,guid,title,content,link,published_at) VALUES(1,1,'one','searchneedle','<p>bodyneedle</p><img src=x onerror=alert(1)>','http://127.0.0.1/private','2026-09-01T12:00:00.000Z')",[])?;
   Ok(())
  }).await.unwrap();
  }
}

#[tokio::test]
async fn protected_rpc_requires_session_and_headers_block_inline_scripts() {
  let test = TestApp::new().await;
  let response = test
    .app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/rpc/article_list")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
  assert!(response.headers()["content-security-policy"]
    .to_str()
    .unwrap()
    .contains("script-src 'self'"));
}

#[tokio::test]
async fn subscriptions_edit_clear_overrides_and_restore_deleted_names() {
  let test = TestApp::new().await;
  let group = test.ok("folder_create", json!({"name":"Blogs"})).await;
  let input = json!({"title":"Example","xml_url":"https://fixture.invalid/feed","folder_id":group["id"],"fetch_interval_sec":1800,"use_readability":true});
  let feed = test.ok("feed_create", input.clone()).await;
  assert_eq!(feed["fetch_interval_sec"], 1800);
  let id = feed["id"].clone();
  let edit = json!({"id":id,"title":"Renamed","xml_url":"https://fixture.invalid/new","html_url":null,"folder_id":null,"fetch_interval_sec":null,"use_readability":false});
  let changed = test.ok("feed_update", edit.clone()).await;
  assert!(changed["folder_id"].is_null());
  assert!(changed["fetch_interval_sec"].is_null());
  assert_eq!(changed["xml_url"], "https://fixture.invalid/new");
  let mut invalid = edit.clone();
  invalid["fetch_interval_sec"] = json!(60);
  invalid["title"] = json!("must not save");
  assert_eq!(test.rpc("feed_update", invalid).await.0, StatusCode::BAD_REQUEST);
  assert_eq!(test.ok("feed_get", json!({"id":id})).await["title"], "Renamed");
  test.ok("folder_delete", json!({"id":group["id"]})).await;
  assert_eq!(test.ok("folder_create", json!({"name":"Blogs"})).await["id"], group["id"]);
  test.ok("feed_delete", json!({"id":id})).await;
  let restored = test.ok("feed_create", edit).await;
  assert_eq!(restored["id"], id);
}
#[tokio::test]
async fn unsafe_feed_addresses_are_rejected_before_queueing() {
  let test = TestApp::new().await;
  for url in [
    "http://127.0.0.1/feed",
    "http://169.254.169.254/",
    "http://[::ffff:127.0.0.1]/",
    "https://fixture.invalid:8080/",
  ] {
    assert_eq!(test.rpc("feed_create", json!({"xml_url":url})).await.0, StatusCode::BAD_REQUEST);
  }
  assert_eq!(test.state.scheduler.status().await.queue_len, 0);
}
#[tokio::test]
async fn folder_lifecycle_and_read_state_round_trip() {
  let test = TestApp::new().await;
  test.seed().await;
  let folder = test.ok("folder_create", json!({"name":"Read later"})).await;
  let fav = json!({"folder_id":folder["id"],"article_id":1});
  test.ok("article_favorite", fav.clone()).await;
  test.ok("article_favorite", fav.clone()).await;
  assert_eq!(test.ok("folder_article_list", json!({"folder_id":folder["id"]})).await["total"], 1);
  assert_eq!(test.ok("article_get", json!({"id":1})).await["folder_ids"], json!([folder["id"]]));
  test.ok("article_mark_read", json!({"id":1})).await;
  assert_eq!(test.ok("unread_count", json!({})).await["total"], 0);
  test.ok("article_mark_unread", json!({"id":1})).await;
  assert_eq!(test.ok("unread_count", json!({})).await["total"], 1);
  test.ok("article_unfavorite", fav).await;
  assert_eq!(test.ok("folder_article_list", json!({"folder_id":folder["id"]})).await["total"], 0);
  test
    .ok("folder_rename", json!({"id":folder["id"],"name":"Saved"}))
    .await;
  test.ok("folder_delete", json!({"id":folder["id"]})).await;
  assert_eq!(test.ok("folder_create", json!({"name":"Saved"})).await["id"], folder["id"]);
  assert_eq!(test.ok("article_get", json!({"id":1})).await["id"], 1);
}
#[tokio::test]
async fn legacy_html_is_cleaned_and_unsafe_fulltext_falls_back() {
  let test = TestApp::new().await;
  test.seed().await;
  let article = test.ok("article_get", json!({"id":1})).await;
  assert!(!article["content"].as_str().unwrap().contains("onerror"));
  let result = test.ok("article_fetch_content", json!({"id":1})).await;
  assert_eq!(result["full_content_status"], "failed");
  assert!(result["content"].as_str().unwrap().contains("bodyneedle"));
  assert!(result["full_content"].is_null());
}
#[tokio::test]
async fn search_filters_counts_pagination_and_full_content() {
  let test = TestApp::new().await;
  test.seed().await;
  with_conn(&test.state.db,|c|{
  c.execute("INSERT INTO t_feed(id,user_id,title,xml_url) VALUES(2,1,'Other','https://other.invalid/feed')",[])?;
  for i in 2..27{c.execute("INSERT INTO t_article(id,feed_id,guid,title,content,published_at) VALUES(?1,2,?2,'searchneedle','中文正文内容','2026-09-10T12:00:00.000Z')",rusqlite::params![i,i.to_string()])?;}
  c.execute("UPDATE t_article SET full_content='<p>fullneedle</p>' WHERE id=1",[])?;Ok(())
 }).await.unwrap();
  let all = test
    .ok("search_query", json!({"q":"searchneedle","page_size":20}))
    .await;
  assert_eq!(all["total"], 26);
  assert_eq!(all["items"].as_array().unwrap().len(), 20);
  assert_eq!(
    test
      .ok("search_query", json!({"q":"searchneedle","page":2,"page_size":20}))
      .await["items"]
      .as_array()
      .unwrap()
      .len(),
    6
  );
  assert_eq!(test.ok("search_query", json!({"q":"searchneedle","feed_id":1})).await["total"], 1);
  assert_eq!(
    test
      .ok("search_query", json!({"q":"searchneedle","date_to":"2026-09-05"}))
      .await["total"],
    1
  );
  assert_eq!(test.ok("search_query", json!({"q":"正文"})).await["total"], 25);
  assert_eq!(test.ok("search_query", json!({"q":"fullneedle"})).await["total"], 1);
}
#[tokio::test]
async fn global_interval_is_persisted_and_validated() {
  let test = TestApp::new().await;
  assert_eq!(test.ok("fetch_config_get", json!({})).await["fetch_interval_sec"], 3600);
  test.ok("fetch_config_update", json!({"fetch_interval_sec":7200})).await;
  assert_eq!(test.ok("fetch_config_get", json!({})).await["fetch_interval_sec"], 7200);
  assert_eq!(test.rpc("fetch_config_update", json!({"fetch_interval_sec":1})).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nested_folder_scope_deduplicates_feeds_and_favorites_and_marks_only_its_subtree() {
  let test = TestApp::new().await;
  test.seed().await;
  let root = test.ok("folder_create", json!({"name":"Root"})).await["id"]
    .as_i64()
    .unwrap();
  let child = test.ok("folder_create", json!({"name":"Child","parent_id":root})).await["id"]
    .as_i64()
    .unwrap();
  with_conn(&test.state.db, move |c| {
    c.execute("UPDATE t_feed SET folder_id=?1 WHERE id=1", [child])?;
    c.execute(
      "INSERT INTO t_feed(id,user_id,title,xml_url) VALUES(2,1,'Outside','https://outside.invalid/feed')",
      [],
    )?;
    for id in 2..=3 {
      c.execute(
        "INSERT INTO t_article(id,feed_id,guid,title) VALUES(?1,2,?2,'searchneedle')",
        rusqlite::params![id, id.to_string()],
      )?;
    }
    Ok(())
  })
  .await
  .unwrap();
  for (article, folder) in [(1, root), (1, child), (2, child)] {
    test
      .ok("article_favorite", json!({"article_id":article,"folder_id":folder}))
      .await;
  }
  let list = test.ok("article_list", json!({"folder_id":root,"page_size":1})).await;
  assert_eq!(list["total"], 2);
  assert_eq!(list["has_more"], true);
  assert_eq!(test.ok("folder_article_list", json!({"folder_id":child})).await["total"], 2);
  assert_eq!(test.ok("article_list", json!({"feed_id":1})).await["total"], 1);
  assert_eq!(
    test
      .ok("search_query", json!({"q":"searchneedle","folder_id":root}))
      .await["total"],
    2
  );
  let folders = test.ok("folder_list", json!({})).await;
  assert!(folders["items"]
    .as_array()
    .unwrap()
    .iter()
    .all(|f| f["article_count"] == 2 && f["unread_count"] == 2));
  let counts = test.ok("unread_count", json!({})).await;
  assert_eq!(counts["total"], 3);
  assert!(counts["folders"].as_array().unwrap().iter().all(|f| f["count"] == 2));
  test.ok("article_read_all", json!({"folder_id":root})).await;
  assert_eq!(
    test
      .ok("article_list", json!({"folder_id":root,"unread_only":true}))
      .await["total"],
    0
  );
  assert_eq!(test.ok("article_get", json!({"id":3})).await["is_read"], false);
  test.ok("feed_delete", json!({"id":1})).await;
  assert_eq!(test.ok("folder_article_list", json!({"folder_id":root})).await["total"], 2);
  test
    .ok("article_unfavorite", json!({"article_id":1,"folder_id":root}))
    .await;
  test
    .ok("article_unfavorite", json!({"article_id":1,"folder_id":child}))
    .await;
  assert_eq!(test.ok("folder_article_list", json!({"folder_id":root})).await["total"], 1);
  assert_eq!(
    test
      .ok("search_query", json!({"q":"searchneedle","folder_id":root}))
      .await["total"],
    1
  );
}

#[tokio::test]
async fn folder_moves_enforce_ownership_cycles_sibling_names_and_nonempty_deletion() {
  let test = TestApp::new().await;
  test.seed().await;
  let root = test.ok("folder_create", json!({"name":"Same"})).await["id"]
    .as_i64()
    .unwrap();
  let child = test.ok("folder_create", json!({"name":"Same","parent_id":root})).await["id"]
    .as_i64()
    .unwrap();
  assert_ne!(
    test
      .rpc("folder_create", json!({"name":"Same","parent_id":root}))
      .await
      .1["code"],
    "0"
  );
  for parent in [root, child] {
    assert_eq!(test.rpc("folder_move", json!({"id":root,"parent_id":parent})).await.0, StatusCode::BAD_REQUEST);
  }
  assert_eq!(test.rpc("folder_move", json!({"id":child,"parent_id":null})).await.0, StatusCode::BAD_REQUEST);
  assert_eq!(test.rpc("folder_delete", json!({"id":root})).await.0, StatusCode::BAD_REQUEST);
  with_conn(&test.state.db, move |c| {
    c.execute("INSERT INTO t_folder(id,user_id,name) VALUES(100,2,'Private')", [])?;
    c.execute("UPDATE t_feed SET folder_id=?1 WHERE id=1", [child])?;
    Ok(())
  })
  .await
  .unwrap();
  assert_eq!(test.rpc("folder_delete", json!({"id":child})).await.0, StatusCode::BAD_REQUEST);
  for (rpc, body) in [
    ("folder_move", json!({"id":child,"parent_id":100})),
    ("folder_move", json!({"id":100,"parent_id":root})),
    ("folder_create", json!({"name":"No","parent_id":100})),
    ("folder_rename", json!({"id":100,"name":"No"})),
    ("folder_delete", json!({"id":100})),
    ("article_list", json!({"folder_id":100})),
    ("search_query", json!({"q":"searchneedle","folder_id":100})),
    ("article_read_all", json!({"folder_id":100})),
    ("article_favorite", json!({"article_id":1,"folder_id":100})),
    ("feed_create", json!({"xml_url":"https://new.invalid/feed","folder_id":100})),
  ] {
    assert_ne!(test.rpc(rpc, body).await.1["code"], "0", "{rpc}");
  }
  test.ok("folder_rename", json!({"id":child,"name":"Child"})).await;
  test.ok("folder_move", json!({"id":child,"parent_id":null})).await;
  test.ok("folder_delete", json!({"id":root})).await;
  assert_eq!(test.ok("feed_get", json!({"id":1})).await["folder_id"], child);
}

#[tokio::test]
async fn folder_depth_limit_checks_entire_moved_subtree() {
  let test = TestApp::new().await;
  let mut parent = Value::Null;
  for index in 0..64 {
    parent = test
      .ok("folder_create", json!({"name":format!("Level {index}"),"parent_id":parent}))
      .await["id"]
      .clone();
  }
  assert_eq!(
    test
      .rpc("folder_create", json!({"name":"Too deep","parent_id":parent}))
      .await
      .0,
    StatusCode::BAD_REQUEST
  );
  let root = test.ok("folder_create", json!({"name":"Other"})).await;
  test
    .ok("folder_create", json!({"name":"Child","parent_id":root["id"]}))
    .await;
  assert_eq!(test.rpc("folder_move", json!({"id":root["id"],"parent_id":63})).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn opml_round_trip_preserves_nested_and_empty_folders() {
  let test = TestApp::new().await;
  test.seed().await;
  let root = test.ok("folder_create", json!({"name":"A & B"})).await["id"]
    .as_i64()
    .unwrap();
  let child = test.ok("folder_create", json!({"name":"Child","parent_id":root})).await["id"]
    .as_i64()
    .unwrap();
  test.ok("folder_create", json!({"name":"Empty","parent_id":root})).await;
  with_conn(&test.state.db, move |c| {
    c.execute("UPDATE t_feed SET folder_id=?1 WHERE id=1", [child])?;
    Ok(())
  })
  .await
  .unwrap();
  let content = test.ok("opml_export", json!({})).await["content"]
    .as_str()
    .unwrap()
    .to_owned();
  assert!(content.contains("A &amp; B"));
  // Import into the same library: duplicate feeds skip fetching, nested folders reuse IDs.
  let imported = test.ok("opml_import", json!({"content":content})).await;
  assert_eq!(imported["parsed"], 1);
  assert_eq!(imported["failed"], 0);
  let folders = test.ok("folder_list", json!({})).await;
  assert_eq!(folders["items"].as_array().unwrap().len(), 3);
  assert!(folders["items"]
    .as_array()
    .unwrap()
    .iter()
    .any(|f| f["name"] == "Empty" && f["parent_id"] == root));
  assert_eq!(test.ok("feed_get", json!({"id":1})).await["folder_id"], child);
}
