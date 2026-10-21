use crate::store::Store;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use maud::{html, Markup, DOCTYPE};

pub fn app(store: Store) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/votacoes/{id}", get(vote))
        .with_state(store)
}

async fn home(State(store): State<Store>) -> Response {
    let votes = match tokio::task::spawn_blocking(move || store.votes("", "", "", "", "", 1)).await
    {
        Ok(Ok(votes)) => votes,
        Ok(Err(error)) => return database_error(error),
        Err(error) => return database_error(error),
    };

    page(
        "Votações recentes",
        html! {
            h1 { "Votações recentes" }
            @if votes.is_empty() {
                p { "Ainda não há votações nominais concluídas no índice." }
            } @else {
                ol {
                    @for vote in &votes {
                        li {
                            a href=(format!("/votacoes/{}", vote.id)) {
                                (vote.house) " — " (vote.date) " — " (vote.description)
                            }
                            " · "
                            a href=(vote.source_url) { "Fonte oficial" }
                        }
                    }
                }
            }
        },
    )
}

async fn vote(Path(id): Path<String>, State(store): State<Store>) -> Response {
    let vote = match tokio::task::spawn_blocking(move || store.vote(&id)).await {
        Ok(Ok(Some(vote))) => vote,
        Ok(Ok(None)) => return StatusCode::NOT_FOUND.into_response(),
        Ok(Err(error)) => return database_error(error),
        Err(error) => return database_error(error),
    };

    page(
        &vote.description,
        html! {
            a href="/" { "Votações recentes" }
            h1 { (vote.description) }
            dl {
                dt { "Casa" } dd { (vote.house) }
                dt { "Data" } dd { (vote.date) }
                dt { "Identificação" } dd { (vote.source_id) }
                dt { "Resultado" } dd { (vote.result) }
                @if vote.yes.is_some() || vote.no.is_some() || vote.other.is_some() {
                    dt { "Totais oficiais" }
                    dd {
                        @if let Some(yes) = vote.yes { "Sim: " (yes) " " }
                        @if let Some(no) = vote.no { "Não: " (no) " " }
                        @if let Some(other) = vote.other { "Outros: " (other) }
                    }
                }
            }
            p { "Votos individuais ainda não estão disponíveis nesta versão." }
            p { a href=(vote.source_url) { "Fonte oficial" } }
        },
    )
}

fn page(title: &str, content: Markup) -> Response {
    let markup = html! {
        (DOCTYPE)
        html lang="pt-BR" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Votações legislativas" }
            }
            body {
                main { (content) }
            }
        }
    };
    Html(markup.into_string()).into_response()
}

fn database_error(error: impl std::fmt::Display) -> Response {
    tracing::error!(%error, "falha ao consultar o índice");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}
