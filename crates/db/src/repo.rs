use std::path::PathBuf;

pub struct DbConfig {
    pub data_dir: PathBuf,
    pub url: Option<String>,
    pub auth_token: Option<String>,
}

impl DbConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            url: None,
            auth_token: None,
        }
    }
}
