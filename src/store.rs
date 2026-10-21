use crate::domain::{Vote, VoteDetail};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Store(pub PathBuf);

impl Store {
    pub fn connection(&self) -> Result<Connection> {
        let db = Connection::open(&self.0)?;
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        Ok(db)
    }

    pub fn init(&self) -> Result<()> {
        self.connection()?.execute_batch("\
            CREATE TABLE IF NOT EXISTS votes (
                id TEXT PRIMARY KEY, house TEXT NOT NULL, source_id TEXT NOT NULL,
                date TEXT NOT NULL, description TEXT NOT NULL, result TEXT NOT NULL,
                yes INTEGER, no INTEGER, other INTEGER, source_url TEXT NOT NULL,
                nominal INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS votes_date ON votes(date DESC);
            CREATE TABLE IF NOT EXISTS proposals (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL,
                source_url TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS vote_proposals (
                vote_id TEXT NOT NULL, proposal_id TEXT NOT NULL,
                PRIMARY KEY(vote_id, proposal_id)
            );
            CREATE TABLE IF NOT EXISTS people (
                id TEXT PRIMARY KEY, house TEXT NOT NULL, name TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS registered_votes (
                vote_id TEXT NOT NULL, person_id TEXT NOT NULL,
                PRIMARY KEY(vote_id, person_id)
            );
            CREATE INDEX IF NOT EXISTS registered_person ON registered_votes(person_id);
            CREATE TABLE IF NOT EXISTS details (
                vote_id TEXT PRIMARY KEY, body TEXT NOT NULL, fetched_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_state (
                source TEXT PRIMARY KEY, updated_at TEXT NOT NULL, note TEXT NOT NULL
            );
        ")?;
        Ok(())
    }

    pub fn upsert_vote(db: &Connection, vote: &Vote, nominal: bool) -> Result<()> {
        db.execute("INSERT INTO votes (id,house,source_id,date,description,result,yes,no,other,source_url,nominal)
          VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
          ON CONFLICT(id) DO UPDATE SET date=excluded.date, description=excluded.description,
          result=excluded.result, yes=excluded.yes, no=excluded.no, other=excluded.other,
          source_url=excluded.source_url, nominal=MAX(votes.nominal,excluded.nominal)",
          params![vote.id,vote.house,vote.source_id,vote.date,vote.description,vote.result,
          vote.yes,vote.no,vote.other,vote.source_url,nominal as i32])?;
        Ok(())
    }

    pub fn vote(&self, id: &str) -> Result<Option<Vote>> {
        let db = self.connection()?;
        db.query_row("SELECT id,house,source_id,date,description,result,yes,no,other,source_url FROM votes WHERE id=?1 AND nominal=1", [id], map_vote).optional().map_err(Into::into)
    }

    pub fn votes(&self, query: &str, house: &str, from: &str, to: &str, person: &str, page: i64) -> Result<Vec<Vote>> {
        let db = self.connection()?;
        let text = format!("%{}%", query.to_lowercase());
        let mut stmt = db.prepare("SELECT DISTINCT v.id,v.house,v.source_id,v.date,v.description,v.result,v.yes,v.no,v.other,v.source_url
          FROM votes v LEFT JOIN vote_proposals vp ON vp.vote_id=v.id LEFT JOIN proposals p ON p.id=vp.proposal_id
          WHERE v.nominal=1 AND (?1='' OR lower(v.description) LIKE ?2 OR lower(p.title) LIKE ?2 OR lower(p.description) LIKE ?2)
          AND (?3='' OR v.house=?3) AND (?4='' OR v.date>=?4) AND (?5='' OR v.date<=?5)
          AND (?6='' OR EXISTS (SELECT 1 FROM registered_votes rv WHERE rv.vote_id=v.id AND rv.person_id=?6))
          ORDER BY v.date DESC, v.id DESC LIMIT 30 OFFSET ?7")?;
        let rows=stmt.query_map(params![query,text,house,from,to,person,(page.max(1)-1)*30],map_vote)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn proposals(&self, query: &str) -> Result<Vec<(String,String,String)>> {
        let db=self.connection()?;
        let mut stmt=db.prepare("SELECT id,title,description FROM proposals WHERE lower(title) LIKE ?1 OR lower(description) LIKE ?1 ORDER BY title LIMIT 30")?;
        let rows=stmt.query_map([format!("%{}%",query.to_lowercase())],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn proposal(&self, id: &str) -> Result<Option<(String,String,String,Vec<Vote>)>> {
        let db=self.connection()?;
        let p:Option<(String,String,String)>=db.query_row("SELECT title,description,source_url FROM proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let Some((title,desc,url))=p else{return Ok(None)};
        let mut stmt=db.prepare("SELECT v.id,v.house,v.source_id,v.date,v.description,v.result,v.yes,v.no,v.other,v.source_url FROM votes v JOIN vote_proposals vp ON vp.vote_id=v.id WHERE vp.proposal_id=?1 AND v.nominal=1 ORDER BY v.date DESC")?;
        let votes=stmt.query_map([id],map_vote)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some((title,desc,url,votes)))
    }

    pub fn people(&self, query: &str) -> Result<Vec<(String,String,String)>> {
        let db=self.connection()?;
        let mut stmt=db.prepare("SELECT id,name,house FROM people WHERE lower(name) LIKE ?1 ORDER BY name LIMIT 30")?;
        let rows=stmt.query_map([format!("%{}%",query.to_lowercase())],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn person(&self,id:&str)->Result<Option<(String,String)>> {
        self.connection()?.query_row("SELECT name,house FROM people WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(Into::into)
    }

    pub fn cached(&self,id:&str)->Result<Option<VoteDetail>> {
        let db=self.connection()?;
        let raw:Option<String>=db.query_row("SELECT body FROM details WHERE vote_id=?1",[id],|r|r.get(0)).optional()?;
        raw.map(|s|serde_json::from_str(&s).map_err(Into::into)).transpose()
    }

    pub fn save_detail(&self,detail:&VoteDetail)->Result<()> {
        self.connection()?.execute("INSERT INTO details(vote_id,body,fetched_at) VALUES(?1,?2,?3) ON CONFLICT(vote_id) DO UPDATE SET body=excluded.body,fetched_at=excluded.fetched_at",
        params![detail.vote.id,serde_json::to_string(detail)?,detail.fetched_at])?;
        Ok(())
    }

    pub fn states(&self)->Result<Vec<(String,String,String)>> {
        let db=self.connection()?;
        let mut stmt=db.prepare("SELECT source,updated_at,note FROM sync_state ORDER BY source")?;
        stmt.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn mark(&self,source:&str,note:&str)->Result<()> {
        self.connection()?.execute("INSERT INTO sync_state(source,updated_at,note) VALUES(?1,datetime('now'),?2) ON CONFLICT(source) DO UPDATE SET updated_at=excluded.updated_at,note=excluded.note",params![source,note])?;
        Ok(())
    }
}

fn map_vote(r:&rusqlite::Row<'_>)->rusqlite::Result<Vote>{
    Ok(Vote{id:r.get(0)?,house:r.get(1)?,source_id:r.get(2)?,date:r.get(3)?,description:r.get(4)?,result:r.get(5)?,yes:r.get(6)?,no:r.get(7)?,other:r.get(8)?,source_url:r.get(9)?})
}
