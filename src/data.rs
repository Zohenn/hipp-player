use crate::dbg_file;
use std::fmt::Display;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir().unwrap().join("hipp-player")
}

pub fn log_error(val: impl Display) {
    let error_file_path = app_data_dir().join("error.log");
    dbg_file!(error_file_path, val);
}
