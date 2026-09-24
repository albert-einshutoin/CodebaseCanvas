use codebasecanvas_analyzer::{discovery::RepositoryRoot, pipeline};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Repo(pub PathBuf);

impl Repo {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-issue18-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub fn copy_fixture(&self) {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample");
        fn copy_sources(source: &Path, target: &Path) {
            fs::create_dir(target).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let dest = target.join(entry.file_name());
                if path.is_dir() {
                    copy_sources(&path, &dest);
                } else if matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("ts" | "tsx" | "prisma")
                ) {
                    fs::copy(path, dest).unwrap();
                }
            }
        }
        copy_sources(&fixture.join("src"), &self.0.join("src"));
        copy_sources(&fixture.join("prisma"), &self.0.join("prisma"));
        fs::copy(fixture.join("tsconfig.json"), self.0.join("tsconfig.json")).unwrap();
    }

    pub fn write(&self, path: &str, content: &str) {
        let target = self.0.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }

    pub fn analyze(&self) -> pipeline::Analysis {
        self.analyze_at("2026-09-24T00:00:00Z")
    }

    pub fn analyze_at(&self, timestamp: &str) -> pipeline::Analysis {
        pipeline::analyze(&RepositoryRoot::open(&self.0).unwrap(), timestamp).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
