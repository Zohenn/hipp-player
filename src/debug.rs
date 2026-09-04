#[macro_export]
macro_rules! dbg_file {
    ($val:expr) => {
        $crate::dbg_file!("/tmp/hipp-player-debug.txt", $val)
    };
    ($file_path:expr, $val:expr) => {
        if let ::std::result::Result::Ok(mut file) = ::std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open($file_path)
        {
            use ::std::io::Write as _;
            let _ = ::std::writeln!(file, "{}", $val);
        }
    };
}
