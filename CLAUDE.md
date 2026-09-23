In the sandbox `/tmp` write access is forbidden, `cargo build` needs `TMPDIR=/tmp/claude-1000 cargo build`

For inserts/updates, don't use `strftime(..., 'now')` in SQL for current time — bind `Utc::now()` as a query param instead.
