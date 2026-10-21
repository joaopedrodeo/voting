use crate::{domain::Vote, store::Store};
use anyhow::{bail, Context, Result};
use chrono::{Duration, NaiveDate};
use reqwest::{header::RETRY_AFTER, Client, StatusCode};
use serde::Deserialize;
use std::time::Duration as Timeout;

const FIRST_DATE: &str = "2023-01-01";
const PERIOD_DAYS: i64 = 7;
const REQUEST_SPACING: Timeout = Timeout::from_millis(120);
const OFFICIAL_VOTATION_URL: &str = "https://legis.senado.leg.br/dadosabertos/votacao";

/// Imports recent public, completed nominal Senate plenary votes.
#[derive(Clone)]
pub struct Senado {
    client: Client,
    base_url: String,
}

impl Senado {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Timeout::from_secs(30))
                .user_agent("voting/0.1 (consulta legislativa local)")
                .build()
                .expect("valid HTTP client"),
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn sync_recent(&self, store: &Store, today: NaiveDate) -> Result<()> {
        let first = NaiveDate::parse_from_str(FIRST_DATE, "%Y-%m-%d")?;
        if today < first {
            return Ok(());
        }
        let from = (today - Duration::days(30)).max(first);
        let mut start = from;
        let result = async {
            while start <= today {
                let end = (start + Duration::days(PERIOD_DAYS - 1)).min(today);
                self.sync_period(store, start, end).await?;
                start = end + Duration::days(1);
            }
            Ok(())
        }
        .await;
        let note = match &result {
            Ok(()) => "votações recentes atualizadas".to_owned(),
            Err(error) => format!("falha ao atualizar votações recentes: {error:#}"),
        };
        store.mark("senado", &note)?;
        result
    }

    async fn sync_period(&self, store: &Store, from: NaiveDate, to: NaiveDate) -> Result<()> {
        let response = self.request(from, to).await?;
        let votes: Vec<SenateVoting> = response
            .json()
            .await
            .with_context(|| format!("Senado: resposta de {from} a {to}"))?;
        for candidate in votes {
            if let Some(vote) = candidate.into_vote(from, to) {
                Store::upsert_vote(&store.connection()?, &vote, true, true)?;
            }
        }
        Ok(())
    }

    async fn request(&self, from: NaiveDate, to: NaiveDate) -> Result<reqwest::Response> {
        let url = format!("{}/votacao", self.base_url);
        for attempt in 0..3 {
            // At most eight requests per second, including retries; the API's
            // documented threshold is ten requests per second.
            tokio::time::sleep(REQUEST_SPACING).await;
            let response = match self
                .client
                .get(&url)
                .header("Accept", "application/json")
                .query(&[("dataInicio", from), ("dataFim", to)])
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) if attempt < 2 && (error.is_connect() || error.is_timeout()) => {
                    tokio::time::sleep(Timeout::from_millis(250 * (attempt + 1))).await;
                    continue;
                }
                Err(error) => return Err(error).with_context(|| format!("Senado: {from} a {to}")),
            };
            if (response.status() == StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error())
                && attempt < 2
            {
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(Timeout::from_secs);
                tokio::time::sleep(
                    retry_after.unwrap_or_else(|| Timeout::from_millis(250 * (attempt + 1))),
                )
                .await;
                continue;
            }
            return response
                .error_for_status()
                .with_context(|| format!("Senado: {from} a {to}"));
        }
        bail!("Senado: falha após tentativas de {from} a {to}")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SenateVoting {
    codigo_sessao: Option<i64>,
    codigo_sessao_votacao: Option<i64>,
    casa_sessao: Option<String>,
    data_sessao: Option<String>,
    votacao_secreta: Option<String>,
    descricao_votacao: Option<String>,
    resultado_votacao: Option<String>,
    total_votos_sim: Option<i64>,
    total_votos_nao: Option<i64>,
    total_votos_abstencao: Option<i64>,
    informe_legislativo: Option<LegislativeReport>,
    votos: Option<Vec<IndividualVote>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegislativeReport {
    sigla_colegiado: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndividualVote {
    codigo_parlamentar: Option<i64>,
    sigla_voto_parlamentar: Option<String>,
}

impl SenateVoting {
    fn into_vote(self, from: NaiveDate, to: NaiveDate) -> Option<Vote> {
        let date = NaiveDate::parse_from_str(self.data_sessao.as_deref()?, "%Y-%m-%d").ok()?;
        let result = self.resultado_votacao?;
        if date < from
            || date > to
            || self.casa_sessao.as_deref() != Some("SF")
            || self.votacao_secreta.as_deref() != Some("N")
            // /votacao replaces the former Plenário nominal listing. Some
            // valid plenary records (notably RQS votes) omit this report.
            || self
                .informe_legislativo
                .as_ref()
                .and_then(|report| report.sigla_colegiado.as_deref())
                .is_some_and(|body| body != "PLEN")
            || !matches!(result.as_str(), "A" | "R")
            || self.votos.as_ref().is_none_or(|votes| {
                !votes.iter().any(|vote| {
                    vote.codigo_parlamentar.is_some()
                        && vote
                            .sigla_voto_parlamentar
                            .as_deref()
                            .is_some_and(|choice| !choice.trim().is_empty())
                })
            })
        {
            return None;
        }
        let id = self.codigo_sessao_votacao?;
        let session_id = self.codigo_sessao?;
        let description = self.descricao_votacao?.trim().to_owned();
        if description.is_empty() {
            return None;
        }
        Some(Vote {
            id: format!("senado:{id}"),
            house: "Senado".to_owned(),
            source_id: id.to_string(),
            date: date.to_string(),
            description,
            result,
            yes: self.total_votos_sim,
            no: self.total_votos_nao,
            other: self.total_votos_abstencao,
            source_url: format!("{OFFICIAL_VOTATION_URL}?codigoSessao={session_id}"),
        })
    }
}
