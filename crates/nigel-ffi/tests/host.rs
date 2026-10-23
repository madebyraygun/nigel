//! Drives `NigelHost` the way the Swift shell does — plain records in, plain
//! records out, awaited from an executor that is not tokio — over real
//! temporary databases, with no webview and no port.

use std::path::Path;
use std::sync::Arc;

use nigel_core::server::{testutil, uploads};
use nigel_ffi::{FfiError, NigelHost, SchemeHeader, SchemeRequest, SchemeResponse};

/// The books folder `seeded_db` made, opened the way the shell opens it.
fn host_over(db_path: &Path) -> Arc<NigelHost> {
    let folder = db_path.parent().expect("a books folder");
    NigelHost::new(Some(folder.display().to_string())).expect("open the books")
}

fn header(name: &str, value: &str) -> SchemeHeader {
    SchemeHeader {
        name: name.to_string(),
        value: value.to_string(),
    }
}

/// What WKWebView hands the scheme handler: the whole URL, and no `Host`.
fn scheme_request(method: &str, path: &str) -> SchemeRequest {
    SchemeRequest {
        method: method.to_string(),
        url: format!(
            "{}{}",
            nigel_core::server::scheme::scheme_url(),
            path.trim_start_matches('/')
        ),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

fn post_json(path: &str, body: &str) -> SchemeRequest {
    let mut request = scheme_request("POST", path);
    request
        .headers
        .push(header("content-type", "application/json"));
    request.body = body.as_bytes().to_vec();
    request
}

fn send(host: &NigelHost, request: SchemeRequest) -> SchemeResponse {
    pollster::block_on(host.request(request))
}

fn header_value<'a>(response: &'a SchemeResponse, name: &str) -> &'a str {
    response
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
        .unwrap_or_default()
}

fn json(response: &SchemeResponse) -> serde_json::Value {
    serde_json::from_slice(&response.body).unwrap_or_else(|e| {
        panic!(
            "{e}: status {} body {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )
    })
}

/// Three rows a built-in importer can read, in the layout `seeded_db`'s
/// checking account is set up for.
fn statement(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("cedar-april-2025.csv");
    std::fs::write(
        &path,
        "Date,Description,Amount,Running Bal.\n\
         04/01/2025,CEDAR SYSTEMS INVOICE 002,1250.00,0.00\n\
         04/03/2025,JUNIPER LABS HOSTING,-84.50,0.00\n\
         04/09/2025,HARBOR AND VALE RETAINER,-119.00,0.00\n",
    )
    .expect("write statement");
    path
}

// -- GET ---------------------------------------------------------------------

#[test]
fn get_root_answers_the_spa_shell() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let response = send(&host, scheme_request("GET", "/"));

    assert_eq!(response.status, 200);
    assert!(
        header_value(&response, "content-type").starts_with("text/html"),
        "{:?}",
        response.headers
    );
    let body = String::from_utf8_lossy(&response.body).to_lowercase();
    assert!(
        body.contains("<!doctype html") && body.contains("<title>nigel</title>"),
        "the body did not look like the SPA shell: {body}"
    );
}

#[test]
fn get_an_api_route_answers_json_from_the_books_with_no_session() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let response = send(&host, scheme_request("GET", "/api/accounts"));

    assert_eq!(response.status, 200);
    assert!(header_value(&response, "content-type").starts_with("application/json"));
    let accounts = json(&response);
    let names: Vec<_> = accounts
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert!(names.contains(&"BofA Checking"), "{accounts}");
}

#[test]
fn a_request_for_another_authority_is_refused() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let mut request = scheme_request("GET", "/api/accounts");
    request.url = "nigel://evil.example/api/accounts".to_string();
    let response = send(&host, request);

    assert_eq!(response.status, 403);
}

#[test]
fn a_request_carrying_a_foreign_host_header_is_refused() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let mut request = scheme_request("GET", "/");
    request.headers.push(header("host", "evil.example"));
    let response = send(&host, request);

    assert_eq!(response.status, 403);
}

#[test]
fn a_subresource_carrying_the_pages_own_origin_is_answered() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let mut request = scheme_request("GET", "/api/accounts");
    request.headers.push(header(
        "origin",
        &nigel_core::server::scheme::scheme_origin(),
    ));
    let response = send(&host, request);

    assert_eq!(response.status, 200);
}

// -- JSON POST ---------------------------------------------------------------

#[test]
fn a_json_post_writes_through_the_router_and_reads_back() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let created = send(
        &host,
        post_json(
            "/api/accounts",
            r#"{"name":"Globex Card","accountType":"credit_card"}"#,
        ),
    );

    assert_eq!(
        created.status,
        201,
        "{}",
        String::from_utf8_lossy(&created.body)
    );
    assert_eq!(json(&created)["name"], "Globex Card");

    let listed = json(&send(&host, scheme_request("GET", "/api/accounts")));
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["name"] == "Globex Card"),
        "{listed}"
    );
}

#[test]
fn a_json_post_the_route_refuses_answers_its_error_envelope() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let response = send(&host, post_json("/api/unlock", r#"{"password":"x"}"#));

    // An unencrypted database needs no password: the route answers 400 with
    // the API's error envelope rather than a transport-level failure.
    assert_eq!(response.status, 400);
    assert!(json(&response)["error"].is_object());
}

// -- multipart ---------------------------------------------------------------

#[test]
fn a_multipart_upload_spools_the_file_and_answers_its_id() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let (content_type, body) =
        testutil::multipart_body("file", "cedar-april-2025.csv", b"Date,Description\n");
    let mut request = scheme_request("POST", "/api/imports/upload");
    request.headers.push(header("content-type", &content_type));
    request.body = body;
    let response = send(&host, request);

    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    let uploaded = json(&response);
    assert_eq!(uploaded["filename"], "cedar-april-2025.csv");
    let id = uploaded["uploadId"].as_str().expect("an uploadId");
    assert!(uploads::resolve(&uploads::uploads_dir(&db_path), id).is_some());
}

// -- staging -----------------------------------------------------------------

#[test]
fn a_staged_file_answers_what_a_browser_upload_of_it_answers() {
    let (dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);
    let source = statement(dir.path());

    let staged = host
        .stage_file(source.display().to_string())
        .expect("stage");

    let bytes = std::fs::read(&source).unwrap();
    let (content_type, body) = testutil::multipart_body("file", "cedar-april-2025.csv", &bytes);
    let mut request = scheme_request("POST", "/api/imports/upload");
    request.headers.push(header("content-type", &content_type));
    request.body = body;
    let uploaded = json(&send(&host, request));

    assert_eq!(uploaded["filename"], staged.filename.as_str());
    assert_eq!(uploaded["size"], staged.size);
    assert_eq!(staged.path, source.display().to_string());
    let spool = uploads::uploads_dir(&db_path);
    let spooled = uploads::resolve(&spool, &staged.upload_id).expect("in the spool");
    assert_eq!(std::fs::read(spooled.path).unwrap(), bytes);
}

#[test]
fn a_staged_file_previews_and_confirms_like_an_uploaded_one() {
    let (dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);
    let source = statement(dir.path());

    let staged = host
        .stage_file(source.display().to_string())
        .expect("stage");
    let body = format!(
        r#"{{"uploadId":"{}","account":"BofA Checking"}}"#,
        staged.upload_id
    );

    let preview = send(&host, post_json("/api/imports/preview", &body));
    assert_eq!(
        preview.status,
        200,
        "{}",
        String::from_utf8_lossy(&preview.body)
    );
    let preview = json(&preview);
    assert_eq!(preview["format"], "bofa_checking");
    assert_eq!(preview["imported"], 3);

    let confirmed = send(&host, post_json("/api/imports/confirm", &body));
    assert_eq!(confirmed.status, 200);
    assert_eq!(json(&confirmed)["imported"], 3);
    assert!(
        uploads::resolve(&uploads::uploads_dir(&db_path), &staged.upload_id).is_none(),
        "the staged file outlived its confirm"
    );
}

#[test]
fn a_file_no_importer_reads_is_refused_with_the_upload_message() {
    let (dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);
    let source = dir.path().join("notes.txt");
    std::fs::write(&source, "not a statement").unwrap();

    let error = host
        .stage_file(source.display().to_string())
        .expect_err("refused");

    let FfiError::Stage { message } = error else {
        panic!("not a staging error: {error:?}");
    };
    assert!(message.contains("notes.txt"), "{message}");
}

// -- the books folder --------------------------------------------------------

#[test]
fn the_host_reports_the_folder_it_was_given() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    assert_eq!(
        host.data_dir(),
        db_path.parent().unwrap().display().to_string()
    );
}

#[test]
fn a_books_folder_that_is_not_there_is_refused_rather_than_created() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("moved-away");

    let error = NigelHost::new(Some(missing.display().to_string()))
        .err()
        .expect("refused");

    assert!(
        matches!(error, FfiError::BooksFolderMissing { .. }),
        "{error:?}"
    );
    assert!(!missing.exists(), "the host created the folder it refused");
}

#[test]
fn a_host_released_from_inside_an_async_context_does_not_panic() {
    let (_dir, db_path) = testutil::seeded_db();
    let host = host_over(&db_path);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async move { drop(host) });
}
