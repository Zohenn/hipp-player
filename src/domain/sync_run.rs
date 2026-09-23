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
