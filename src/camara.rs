use crate::{domain::Vote, store::Store};
use anyhow::{bail, Context, Result};
use chrono::{Duration, NaiveDate};
use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use std::{collections::HashMap, time::Duration as Timeout};

const PLENARY_ID: &str = "180";
const FIRST_DATE: &str = "2023-01-01";
// The API has no reliable nominal/symbolic flag. A substantial individual
// ballot list and official tally are evidence when prose omits the modality.
const MIN_VOTES_WITHOUT_LABEL: usize = 50;

/// Imports recent completed nominal plenary votes into the compact index.
#[derive(Clone)]
pub struct Camara {
    client: Client,
    base_url: String,
    archive_base_url: String,
}

impl Camara {
    pub fn new(base_url: String, archive_base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Timeout::from_secs(15))
                .user_agent("voting/0.1 (consulta legislativa local)")
                .build()
                .expect("valid HTTP client"),
            base_url: base_url.trim_end_matches('/').to_owned(),
            archive_base_url: archive_base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn sync_recent(&self, store: &Store, today: NaiveDate) -> Result<()> {
        let first = NaiveDate::parse_from_str(FIRST_DATE, "%Y-%m-%d")?;
        if today < first {
            return Ok(());
        }
        let from = (today - Duration::days(30)).max(first);
        let result = self.sync_range(store, from, today).await;
        let note = match &result {
            Ok(()) => "votações recentes atualizadas".to_owned(),
            Err(error) => format!("falha ao atualizar votações recentes: {error:#}"),
        };
        store.mark("camara", &note)?;
        result
    }

    async fn sync_range(&self, store: &Store, from: NaiveDate, to: NaiveDate) -> Result<()> {
        let mut start = from;
        while start <= to {
            let end_of_year = NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
                .context("invalid year")?
                - Duration::days(1);
            let end = end_of_year.min(to);
            self.sync_period(store, start, end).await?;
            start = end + Duration::days(1);
        }
        Ok(())
    }

    async fn sync_period(&self, store: &Store, from: NaiveDate, to: NaiveDate) -> Result<()> {
        let totals = self.official_totals(from.year()).await?;
        let mut page = 1;
        loop {
            let response: Page<CamaraVoting> = self
                .get(
                    &format!("{}/votacoes", self.base_url),
                    &[
                        ("dataInicio", from.to_string()),
                        ("dataFim", to.to_string()),
                        ("idOrgao", PLENARY_ID.to_owned()),
                        ("itens", "100".to_owned()),
                        ("pagina", page.to_string()),
                        ("ordem", "DESC".to_owned()),
                        ("ordenarPor", "data".to_owned()),
                    ],
                )
                .await
                .with_context(|| format!("Câmara: página {page} de {from} a {to}"))?;

            for candidate in response.dados {
                if candidate.sigla_orgao != "PLEN"
                    || !candidate.date_in(from, to)
                    || mentions_symbolic(&candidate.descricao)
                {
                    continue;
                }
                let detail: Envelope<CamaraDetail> = self
                    .get(&format!("{}/votacoes/{}", self.base_url, candidate.id), &[])
                    .await
                    .with_context(|| format!("Câmara: detalhe {}", candidate.id))?;
                if detail.dados.sigla_orgao != "PLEN"
                    || mentions_symbolic(&detail.dados.descricao)
                    || detail
                        .dados
                        .opening
                        .as_deref()
                        .is_some_and(mentions_symbolic)
                {
                    continue;
                }
                let registered_votes: Envelope<Vec<serde_json::Value>> = self
                    .get(
                        &format!("{}/votacoes/{}/votos", self.base_url, candidate.id),
                        &[],
                    )
                    .await
                    .with_context(|| format!("Câmara: votos {}", candidate.id))?;
                if registered_votes.dados.is_empty() {
                    continue;
                }
                let official = totals.get(&candidate.id);
                let has_nominal_evidence = detail
                    .dados
                    .opening
                    .as_deref()
                    .is_some_and(mentions_nominal)
                    || mentions_nominal(&candidate.descricao)
                    || (parse_labeled_total(&candidate.descricao, "Sim:").is_some()
                        && parse_labeled_total(&candidate.descricao, "Não:").is_some())
                    || (registered_votes.dados.len() >= MIN_VOTES_WITHOUT_LABEL
                        && official.is_some_and(OfficialTotals::has_substantial_tally));
                if !has_nominal_evidence {
                    // Câmara publishes plenary orientations only for nominal votes.
                    let orientations: Envelope<Vec<serde_json::Value>> = self
                        .get(
                            &format!("{}/votacoes/{}/orientacoes", self.base_url, candidate.id),
                            &[],
                        )
                        .await
                        .with_context(|| format!("Câmara: orientações {}", candidate.id))?;
                    if orientations.dados.is_empty() {
                        continue;
                    }
                }
                let item = detail
                    .dados
                    .opening
                    .filter(|text| !text.trim().is_empty())
                    .or_else(|| candidate.object.filter(|text| !text.trim().is_empty()))
                    .unwrap_or_else(|| "Item votado não identificado pela fonte".to_owned());
                let vote = Vote {
                    id: format!("camara:{}", candidate.id),
                    house: "Câmara".to_owned(),
                    source_id: candidate.id,
                    date: candidate.data,
                    description: item,
                    result: candidate.descricao.clone(),
                    yes: official
                        .and_then(|totals| totals.yes)
                        .or_else(|| parse_labeled_total(&candidate.descricao, "Sim:")),
                    no: official
                        .and_then(|totals| totals.no)
                        .or_else(|| parse_labeled_total(&candidate.descricao, "Não:")),
                    other: official
                        .and_then(|totals| totals.other)
                        .or_else(|| parse_labeled_total(&candidate.descricao, "Outros:")),
                    source_url: candidate.uri,
                };
                let db = store.connection()?;
                Store::upsert_vote(&db, &vote, true, true)?;
            }
            if !response.links.iter().any(|link| link.rel == "next") {
                break;
            }
            page += 1;
        }
        Ok(())
    }

    async fn official_totals(&self, year: i32) -> Result<HashMap<String, OfficialTotals>> {
        let url = format!("{}/votacoes-{year}.csv", self.archive_base_url);
        let bytes = self.request(&url, &[]).await?.bytes().await?;
        let raw = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(raw);
        let mut totals = HashMap::new();
        for row in reader.deserialize::<CamaraTotalsRow>() {
            let row = row.with_context(|| format!("Câmara: totais de {year}"))?;
            totals.insert(
                row.id,
                OfficialTotals {
                    yes: row.yes.parse().ok(),
                    no: row.no.parse().ok(),
                    other: row.other.parse().ok(),
                },
            );
        }
        Ok(totals)
    }

    async fn get<T: DeserializeOwned>(&self, url: &str, query: &[(&str, String)]) -> Result<T> {
        Ok(self.request(url, query).await?.json().await?)
    }

    async fn request(&self, url: &str, query: &[(&str, String)]) -> Result<reqwest::Response> {
        for attempt in 0..3 {
            let response = match self.client.get(url).query(query).send().await {
                Ok(response) => response,
                Err(error) if attempt < 2 && (error.is_connect() || error.is_timeout()) => {
                    tokio::time::sleep(Timeout::from_millis(200 * (attempt + 1))).await;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if response.status() == StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error()
            {
                if attempt < 2 {
                    tokio::time::sleep(Timeout::from_millis(200 * (attempt + 1))).await;
                    continue;
                }
            }
            return Ok(response.error_for_status()?);
        }
        bail!("Câmara: failed after retries")
    }
}

use chrono::Datelike;

#[derive(Deserialize)]
struct Envelope<T> {
    dados: T,
}

#[derive(Deserialize)]
struct Page<T> {
    dados: Vec<T>,
    links: Vec<Link>,
}

#[derive(Deserialize)]
struct Link {
    rel: String,
}

#[derive(Deserialize)]
struct CamaraVoting {
    id: String,
    uri: String,
    data: String,
    #[serde(rename = "siglaOrgao")]
    sigla_orgao: String,
    descricao: String,
    #[serde(rename = "proposicaoObjeto")]
    object: Option<String>,
}

impl CamaraVoting {
    fn date_in(&self, from: NaiveDate, to: NaiveDate) -> bool {
        NaiveDate::parse_from_str(&self.data, "%Y-%m-%d")
            .is_ok_and(|date| date >= from && date <= to)
    }
}

#[derive(Deserialize)]
struct CamaraDetail {
    #[serde(rename = "siglaOrgao")]
    sigla_orgao: String,
    descricao: String,
    #[serde(rename = "descUltimaAberturaVotacao")]
    opening: Option<String>,
}

#[derive(Deserialize)]
struct CamaraTotalsRow {
    id: String,
    #[serde(rename = "votosSim")]
    yes: String,
    #[serde(rename = "votosNao")]
    no: String,
    #[serde(rename = "votosOutros")]
    other: String,
}

struct OfficialTotals {
    yes: Option<i64>,
    no: Option<i64>,
    other: Option<i64>,
}

impl OfficialTotals {
    fn has_substantial_tally(&self) -> bool {
        self.yes.unwrap_or_default() + self.no.unwrap_or_default() + self.other.unwrap_or_default()
            >= MIN_VOTES_WITHOUT_LABEL as i64
    }
}

fn mentions_symbolic(text: &str) -> bool {
    text.to_lowercase().contains("simbólic") || text.to_lowercase().contains("simbolic")
}

fn mentions_nominal(text: &str) -> bool {
    text.to_lowercase().contains("nominal")
}

fn parse_labeled_total(text: &str, label: &str) -> Option<i64> {
    let value = text.split(label).nth(1)?.trim_start();
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
