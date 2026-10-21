use axum::{
    body::{to_bytes, Body},
    extract::{Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tower::ServiceExt;
use voting::{app, domain::Vote, senado::Senado, store::Store};

static NEXT_DATABASE: AtomicUsize = AtomicUsize::new(0);

struct TestDb {
    dir: PathBuf,
    store: Store,
}

impl TestDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "voting-senado-test-{}-{}",
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

fn voting(id: i64, date: &str, item: &str) -> Value {
    json!({
        "codigoSessao": 526731,
        "codigoSessaoVotacao": id,
        "casaSessao": "SF",
        "dataSessao": date,
        "votacaoSecreta": "N",
        "descricaoVotacao": item,
        "resultadoVotacao": "A",
        "totalVotosSim": 42,
        "totalVotosNao": 3,
        "totalVotosAbstencao": 1,
        "informeLegislativo": {"siglaColegiado": "PLEN"},
        "votos": [{"codigoParlamentar": 123, "siglaVotoParlamentar": "Sim"}]
    })
}

#[derive(Clone)]
struct SourceState {
    requests: Arc<Mutex<Vec<(NaiveDate, NaiveDate)>>>,
    calls: Arc<AtomicUsize>,
    fail_after_first_period: bool,
}

async fn source(
    fail_after_first_period: bool,
) -> (String, SourceState, tokio::task::JoinHandle<()>) {
    async fn list(
        Query(query): Query<HashMap<String, String>>,
        State(state): State<SourceState>,
    ) -> Response {
        let from = NaiveDate::parse_from_str(query.get("dataInicio").unwrap(), "%Y-%m-%d").unwrap();
        let to = NaiveDate::parse_from_str(query.get("dataFim").unwrap(), "%Y-%m-%d").unwrap();
        state.requests.lock().unwrap().push((from, to));
        assert!(to >= from);
        assert!((to - from).num_days() < 7);

        if state.fail_after_first_period && from >= NaiveDate::from_ymd_opt(2024, 12, 18).unwrap() {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        if !state.fail_after_first_period && state.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }

        let mut data = Vec::new();
        if from <= NaiveDate::from_ymd_opt(2024, 12, 12).unwrap()
            && to >= NaiveDate::from_ymd_opt(2024, 12, 12).unwrap()
        {
            data.push(voting(100, "2024-12-12", "Votação de dezembro"));
        }
        if from <= NaiveDate::from_ymd_opt(2025, 1, 8).unwrap()
            && to >= NaiveDate::from_ymd_opt(2025, 1, 8).unwrap()
        {
            data.push(voting(200, "2025-01-08", "Votação de janeiro"));
            data.push(voting(200, "2025-01-08", "Votação de janeiro"));
            let mut no_report = voting(206, "2025-01-08", "Requerimento sem informe");
            no_report["informeLegislativo"] = Value::Null;
            data.push(no_report);
            let mut secret = voting(201, "2025-01-08", "Votação secreta");
            secret["votacaoSecreta"] = json!("S");
            data.push(secret);
            let mut other_house = voting(202, "2025-01-08", "Congresso Nacional");
            other_house["casaSessao"] = json!("CN");
            data.push(other_house);
            let mut committee = voting(203, "2025-01-08", "Comissão");
            committee["informeLegislativo"]["siglaColegiado"] = json!("CCJ");
            data.push(committee);
            let mut unfinished = voting(204, "2025-01-08", "Sem resultado");
            unfinished["resultadoVotacao"] = Value::Null;
            data.push(unfinished);
            let mut no_individual_votes = voting(205, "2025-01-08", "Sem votos publicados");
            no_individual_votes["votos"] = json!([]);
            data.push(no_individual_votes);
            let mut incomplete_votes = voting(207, "2025-01-08", "Votos sem parlamentar");
            incomplete_votes["votos"] = json!([{}]);
            data.push(incomplete_votes);
        }
        Json(data).into_response()
    }

    let state = SourceState {
        requests: Arc::new(Mutex::new(Vec::new())),
        calls: Arc::new(AtomicUsize::new(0)),
        fail_after_first_period,
    };
    let router = Router::new()
        .route("/votacao", get(list))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}"), state, handle)
}

#[tokio::test]
async fn homepage_combines_recent_senate_and_camara_votes_in_date_order() {
    let db = TestDb::new();
    Store::upsert_vote(
        &db.store.connection().unwrap(),
        &Vote {
            id: "camara:300".into(),
            house: "Câmara".into(),
            source_id: "300".into(),
            date: "2025-01-09".into(),
            description: "Votação da Câmara".into(),
            result: "Aprovado".into(),
            yes: None,
            no: None,
            other: None,
            source_url: "https://dadosabertos.camara.leg.br/api/v2/votacoes/300".into(),
        },
        true,
        true,
    )
    .unwrap();
    let (base, state, server) = source(false).await;
    let senado = Senado::new(base);
    let today = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
    senado.sync_recent(&db.store, today).await.unwrap();

    let page = render_path(db.store.clone(), "/").await;
    assert!(page.find("Votação da Câmara").unwrap() < page.find("Votação de janeiro").unwrap());
    assert!(page.find("Votação de janeiro").unwrap() < page.find("Votação de dezembro").unwrap());
    assert!(page.contains("https://legis.senado.leg.br/dadosabertos/votacao?codigoSessao=526731"));
    assert!(page.contains("Requerimento sem informe"));
    for excluded in [
        "Votação secreta",
        "Congresso Nacional",
        "Comissão",
        "Sem resultado",
        "Sem votos publicados",
        "Votos sem parlamentar",
    ] {
        assert!(!page.contains(excluded), "{excluded} should be excluded");
    }
    senado.sync_recent(&db.store, today).await.unwrap();
    let detail = render_path(db.store.clone(), "/votacoes/senado:200").await;
    assert!(detail.contains("Votação de janeiro"));
    assert!(detail.contains("Resultado</dt><dd>A"));
    assert!(detail.contains("Sim: 42"));
    assert!(detail.contains("Não: 3"));
    assert!(detail.contains("Outros: 1"));
    assert_eq!(
        render_path(db.store.clone(), "/")
            .await
            .matches("Votação de janeiro")
            .count(),
        1
    );

    let requests = state.requests.lock().unwrap();
    assert!(requests.len() > 2);
    assert!(requests.iter().any(|(from, to)| *from
        <= NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()
        && *to >= NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()));
    server.abort();
}

#[tokio::test]
async fn failed_later_period_preserves_existing_and_earlier_senate_votes() {
    let db = TestDb::new();
    Store::upsert_vote(
        &db.store.connection().unwrap(),
        &Vote {
            id: "senado:older".into(),
            house: "Senado".into(),
            source_id: "older".into(),
            date: "2024-12-11".into(),
            description: "Votação já indexada".into(),
            result: "A".into(),
            yes: None,
            no: None,
            other: None,
            source_url: "https://legis.senado.leg.br/dadosabertos/votacao?codigoSessao=123".into(),
        },
        true,
        true,
    )
    .unwrap();
    let (base, _state, server) = source(true).await;
    let result = Senado::new(base)
        .sync_recent(&db.store, NaiveDate::from_ymd_opt(2025, 1, 10).unwrap())
        .await;
    assert!(result.is_err());
    let page = render_path(db.store.clone(), "/").await;
    assert!(page.contains("Votação já indexada"));
    assert!(page.contains("Votação de dezembro"));
    assert!(!page.contains("Votação de janeiro"));
    server.abort();
}
