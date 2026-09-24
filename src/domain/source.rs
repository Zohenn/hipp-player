use chrono::{DateTime, Utc};

pub struct Source {
    pub id: i64,
    pub kind: SourceKind,
    pub identifier: String,
    pub created_at: DateTime<Utc>,
}

pub enum SourceKind {
    OpenSubsonic,
}

impl SourceKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SourceKind::OpenSubsonic => "opensubsonic",
        }
    }
}
