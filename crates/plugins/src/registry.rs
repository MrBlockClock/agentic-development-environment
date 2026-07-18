use std::path::PathBuf;

pub struct PluginRegistry {
    pub dirs: Vec<PathBuf>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { dirs: vec![] }
    }

    pub fn discover(&self) -> Vec<String> {
        // TODO: scan registry dirs for plugin manifests
        vec![]
    }
}
