use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use voting::{app, domain::Vote, store::Store};

static NEXT_DATABASE: AtomicUsize = AtomicUsize::new(0);

struct TestDb {
    dir: PathBuf,
    store: Store,
}

impl TestDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "voting-home-test-{}-{}",
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

async fn get(store: Store, path: &str) -> (StatusCode, String) {
    let response = app(store)
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn indexed_vote(id: &str, date: &str, house: &str) -> Vote {
    Vote {
        id: id.into(),
        house: house.into(),
        source_id: id.into(),
        date: date.into(),
        description: format!("Item votado {id}"),
        result: "Aprovado".into(),
        yes: Some(100),
        no: Some(50),
        other: None,
        source_url: format!("https://example.org/votacoes/{id}"),
    }
}

#[tokio::test]
async fn homepage_handles_an_empty_index() {
    let db = TestDb::new();
    let (status, body) = get(db.store.clone(), "/").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Ainda não há votações nominais concluídas no índice."));
}

#[tokio::test]
async fn homepage_lists_only_completed_nominal_votes_newest_first() {
    let db = TestDb::new();
    let connection = db.store.connection().unwrap();
    Store::upsert_vote(
        &connection,
        &indexed_vote("camara:older", "2025-01-01", "Câmara"),
        true,
        true,
    )
    .unwrap();
    Store::upsert_vote(
        &connection,
        &indexed_vote("senado:newer", "2025-02-01", "Senado"),
        true,
        true,
    )
    .unwrap();
    Store::upsert_vote(
        &connection,
        &indexed_vote("camara:symbolic", "2025-03-01", "Câmara"),
        false,
        true,
    )
    .unwrap();
    Store::upsert_vote(
        &connection,
        &indexed_vote("senado:open", "2025-04-01", "Senado"),
        true,
        false,
    )
    .unwrap();
    drop(connection);

    let (status, body) = get(db.store.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);

    assert!(body.find("senado:newer").unwrap() < body.find("camara:older").unwrap());
    assert!(body.contains("Senado — 2025-02-01 — Item votado senado:newer"));
    assert!(body.contains("https://example.org/votacoes/senado:newer"));
    assert!(!body.contains("camara:symbolic"));
    assert!(!body.contains("senado:open"));

    let (status, detail) = get(db.store.clone(), "/votacoes/senado:newer").await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail.contains("Item votado senado:newer"));
    assert!(detail.contains("Votos individuais ainda não estão disponíveis nesta versão."));
}

#[tokio::test]
async fn homepage_reads_an_index_created_with_the_previous_schema() {
    let db = TestDb::new();
    let legacy = db.store.connection().unwrap();
    legacy
        .execute_batch(
            "DROP TABLE votes;
            CREATE TABLE votes (
                id TEXT PRIMARY KEY, house TEXT NOT NULL, source_id TEXT NOT NULL,
                date TEXT NOT NULL, description TEXT NOT NULL, result TEXT NOT NULL,
                yes INTEGER, no INTEGER, other INTEGER, source_url TEXT NOT NULL,
                nominal INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO votes (id, house, source_id, date, description, result, source_url, nominal)
            VALUES ('camara:legacy', 'Câmara', 'legacy', '2025-01-01', 'Item antigo', 'Aprovado',
                    'https://example.org/votacoes/legacy', 1);",
        )
        .unwrap();
    drop(legacy);
    db.store.init().unwrap();

    let (status, body) = get(db.store.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Item antigo"));
}
