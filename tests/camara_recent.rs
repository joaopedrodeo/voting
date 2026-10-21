use axum::{
    body::{to_bytes, Body},
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use voting::{app, camara::Camara, store::Store};

static NEXT_DATABASE: AtomicUsize = AtomicUsize::new(0);

fn sample_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 12, 17).unwrap()
}

struct TestDb {
    dir: PathBuf,
    store: Store,
}

impl TestDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "voting-camara-test-{}-{}",
            std::process::id(),
            NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let store = Store(dir.join("index.sqlite3"));
        store.init().unwrap();
        Self { dir, store }
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

async fn render_path(store: Store, path: &str) -> String {
    let response = app(store)
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn source(fail_second_page: bool) -> (String, tokio::task::JoinHandle<()>) {
    async fn list(
        Query(query): Query<std::collections::HashMap<String, String>>,
        State(fail_second_page): State<bool>,
    ) -> Response {
        assert_eq!(query.get("idOrgao").map(String::as_str), Some("180"));
        let page = query.get("pagina").map(String::as_str).unwrap_or("1");
        if page == "2" && fail_second_page {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        let result = match page {
            "1" => json!({"dados": [
                {"id":"nominal-new", "data":"2025-12-16", "siglaOrgao":"PLEN", "descricao":"Aprovado o destaque.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/nominal-new"},
                {"id":"symbolic", "data":"2025-12-15", "siglaOrgao":"PLEN", "descricao":"Aprovado em votação simbólica.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/symbolic"}
            ], "links":[{"rel":"next", "href":"/votacoes?pagina=2"}]}),
            "2" => json!({"dados": [
                {"id":"nominal-old", "data":"2025-12-14", "siglaOrgao":"PLEN", "descricao":"Rejeitado o requerimento. Sim: 80; Não: 320; Total: 400.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/nominal-old"},
                {"id":"nominal-unlabeled", "data":"2025-12-14", "siglaOrgao":"PLEN", "descricao":"Mantido o texto.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/nominal-unlabeled"},
                {"id":"nominal-oriented", "data":"2025-12-14", "siglaOrgao":"PLEN", "descricao":"Rejeitado o item.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/nominal-oriented"},
                {"id":"committee", "data":"2025-12-14", "siglaOrgao":"CCJC", "descricao":"Aprovado. Sim: 20; Não: 2.", "uri":"https://dadosabertos.camara.leg.br/api/v2/votacoes/committee"}
            ], "links":[]}),
            _ => panic!("unexpected page {page}"),
        };
        Json(result).into_response()
    }
    async fn detail(Path(id): Path<String>) -> Json<Value> {
        let opening = if id == "nominal-unlabeled" || id == "nominal-oriented" {
            format!("Apreciação do item {id}")
        } else {
            format!("Votação nominal do item {id}")
        };
        Json(
            json!({"dados":{"id":id,"siglaOrgao":"PLEN","descUltimaAberturaVotacao":opening,"descricao":"Resultado registrado"}}),
        )
    }
    async fn votes(Path(id): Path<String>) -> Json<Value> {
        let dados = if id == "nominal-unlabeled" {
            (0..200).map(|_| json!({"tipoVoto":"Sim"})).collect()
        } else if id == "nominal-oriented" {
            (0..3).map(|_| json!({"tipoVoto":"Não"})).collect()
        } else {
            vec![json!({"tipoVoto":"Sim"})]
        };
        Json(json!({"dados":dados}))
    }
    async fn orientations(Path(id): Path<String>) -> Json<Value> {
        let dados = if id == "nominal-oriented" {
            vec![json!({"siglaPartido":"ABC", "orientacao":"Não"})]
        } else {
            vec![]
        };
        Json(json!({"dados":dados}))
    }
    async fn archive() -> &'static str {
        "\u{feff}\"id\";\"votosSim\";\"votosNao\";\"votosOutros\"\n\"nominal-new\";\"300\";\"100\";\"2\"\n\"nominal-old\";\"80\";\"320\";\"0\"\n\"nominal-unlabeled\";\"200\";\"195\";\"5\"\n\"nominal-oriented\";\"0\";\"3\";\"0\"\n"
    }
    let router = Router::new()
        .route("/votacoes", get(list))
        .route("/votacoes/{id}", get(detail))
        .route("/votacoes/{id}/votos", get(votes))
        .route("/votacoes/{id}/orientacoes", get(orientations))
        .route("/arquivos/votacoes/csv/votacoes-2025.csv", get(archive))
        .with_state(fail_second_page);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn homepage_imports_recent_nominal_plenary_votes_from_paginated_camara_source() {
    let db = TestDb::new();
    let (base, server) = source(false).await;
    let camara = Camara::new(base.clone(), format!("{base}/arquivos/votacoes/csv"));
    camara.sync_recent(&db.store, sample_date()).await.unwrap();

    let page = render_path(db.store.clone(), "/").await;
    assert!(page.find("item nominal-new").unwrap() < page.find("item nominal-old").unwrap());
    assert!(page.contains("https://dadosabertos.camara.leg.br/api/v2/votacoes/nominal-new"));
    assert!(!page.contains("symbolic"));
    assert!(!page.contains("committee"));
    assert!(page.contains("item nominal-unlabeled"));
    assert!(page.contains("item nominal-oriented"));
    let detail = render_path(db.store.clone(), "/votacoes/camara:nominal-new").await;
    assert!(detail.contains("Votação nominal do item nominal-new"));
    assert!(detail.contains("2025-12-16"));
    assert!(detail.contains("Aprovado o destaque."));
    assert!(detail.contains("Sim: 300"));
    assert!(detail.contains("Não: 100"));
    assert!(detail.contains("Outros: 2"));
    camara.sync_recent(&db.store, sample_date()).await.unwrap();
    let page = render_path(db.store.clone(), "/").await;
    assert_eq!(page.matches("item nominal-new").count(), 1);
    server.abort();
}

#[tokio::test]
async fn failed_camara_page_keeps_votes_already_imported_visible() {
    let db = TestDb::new();
    let (base, server) = source(true).await;
    let result = Camara::new(base.clone(), format!("{base}/arquivos/votacoes/csv"))
        .sync_recent(&db.store, sample_date())
        .await;
    assert!(result.is_err());

    let page = render_path(db.store.clone(), "/").await;
    assert!(page.contains("item nominal-new"));
    assert!(!page.contains("item nominal-old"));
    server.abort();
}
