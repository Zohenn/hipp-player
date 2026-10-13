use chrono::{DateTime, Utc};
use serde::Deserialize;

pub struct Source {
    pub id: i64,
    pub kind: SourceKind,
    pub identifier: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub enum SourceKind {
    #[serde(rename = "opensubsonic")]
    OpenSubsonic,
}

impl SourceKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SourceKind::OpenSubsonic => "opensubsonic",
        }
    }
}
