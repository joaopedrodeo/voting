use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vote {
    pub id: String,
    pub house: String,
    pub source_id: String,
    pub date: String,
    pub description: String,
    pub result: String,
    pub yes: Option<i64>,
    pub no: Option<i64>,
    pub other: Option<i64>,
    pub source_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonVote {
    pub person_id: String,
    pub name: String,
    pub party: String,
    pub state: String,
    pub choice: String,
    pub group: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteDetail {
    pub vote: Vote,
    pub people: Vec<PersonVote>,
    pub roster_complete: bool,
    pub fetched_at: String,
    pub stale: bool,
    pub warning: Option<String>,
}

pub fn choice_group(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "sim" => "sim",
        "não" | "nao" => "não",
        _ => "outros",
    }
}

pub fn common_proposal_key(kind: &str, number: &str, year: &str) -> Option<String> {
    let kind = kind.trim().to_uppercase();
    let number: u32 = number.trim().parse().ok()?;
    let year: u32 = year.trim().parse().ok()?;
    if year >= 2019 && matches!(kind.as_str(), "PEC" | "PLP" | "PL" | "PDL") {
        Some(format!("{kind}:{number}:{year}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_officially_shared_identifiers_are_merged() {
        assert_eq!(common_proposal_key("PEC", "221", "2019"), Some("PEC:221:2019".into()));
        assert_eq!(common_proposal_key("PEC", "221", "2018"), None);
        assert_eq!(common_proposal_key("REQ", "3", "2025"), None);
    }
    #[test]
    fn unrecognized_vote_stays_in_other() {
        assert_eq!(choice_group("P-NRV"), "outros");
        assert_eq!(choice_group("Não"), "não");
    }
}
