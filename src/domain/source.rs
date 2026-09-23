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
