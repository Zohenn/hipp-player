pub enum SyncRunStatus {
    Success,
    Partial,
    Failed,
}

impl SyncRunStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SyncRunStatus::Success => "success",
            SyncRunStatus::Partial => "partial",
            SyncRunStatus::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncKind {
    Full,
    Incremental,
}

impl SyncKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SyncKind::Full => "full",
            SyncKind::Incremental => "incremental",
        }
    }
}
