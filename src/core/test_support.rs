use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::core::util::timestamp_nanos;

pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "agent-linker-{label}-{}-{}",
            std::process::id(),
            timestamp_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
