use codebasecanvas_analyzer::{SystemGraph, discovery::RepositoryRoot};
use std::path::{Path, PathBuf};

pub struct Repository {
    pub root: RepositoryRoot,
    #[cfg(unix)]
    directory: std::fs::File,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, String> {
        // Capture the selected directory before resolving any symlink/canonical path.
        let before =
            std::fs::metadata(path).map_err(|_| "repository cannot be opened".to_owned())?;
        if !before.is_dir() {
            return Err("repository is not a directory".to_owned());
        }
        #[cfg(unix)]
        {
            Self::open_checked(path, &before)
        }
        #[cfg(not(unix))]
        Err("safe output requires Unix directory-relative I/O".into())
    }

    #[cfg(unix)]
    fn open_checked(path: &Path, before: &std::fs::Metadata) -> Result<Self, String> {
        use std::os::unix::fs::MetadataExt;
        let root = RepositoryRoot::open(path)?;
        let directory = rustix::fs::open(root.path(), unix::DIR_FLAGS, rustix::fs::Mode::empty())
            .map_err(|_| "repository cannot be opened safely".to_owned())?;
        let directory = std::fs::File::from(directory);
        let opened = directory
            .metadata()
            .map_err(|error| format!("repository {path:?}: {error}"))?;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err("repository changed while opening".into());
        }
        Ok(Self { root, directory })
    }

    pub fn save(&self, graph: &SystemGraph) -> Result<PathBuf, String> {
        // Validate and serialize before creating output or touching the previous snapshot.
        let json = graph
            .to_json()
            .map_err(|error| format!("graph validation: {error}"))?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::MetadataExt;
            let named = std::fs::symlink_metadata(self.root.path())
                .map_err(|error| format!("repository {:?}: {error}", self.root.path()))?;
            let opened = self
                .directory
                .metadata()
                .map_err(|error| format!("repository {:?}: {error}", self.root.path()))?;
            if !named.is_dir() || named.dev() != opened.dev() || named.ino() != opened.ino() {
                return Err("repository changed during analysis".into());
            }
            unix::atomic_write(&self.directory, |file| file.write_all(json.as_bytes()))
                .map_err(|error| format!("output in {:?}: {error}", self.root.path()))?;
            Ok(self.root.path().join(".codebasecanvas/graph.json"))
        }
        #[cfg(not(unix))]
        {
            let _ = json;
            Err("safe output requires Unix directory-relative I/O".into())
        }
    }
}

#[cfg(unix)]
mod unix {
    use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};
    use rustix::io::Errno;
    use std::fs::File;
    use std::io;
    use std::os::fd::OwnedFd;
    use std::sync::atomic::{AtomicU64, Ordering};

    const DIRECTORY: &str = ".codebasecanvas";
    const TARGET: &str = "graph.json";
    static NEXT: AtomicU64 = AtomicU64::new(0);
    pub(super) const DIR_FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    fn check_target(directory: &OwnedFd) -> io::Result<()> {
        match fs::statat(directory, TARGET, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => Ok(()),
            Ok(_) => Err(io::Error::other(
                "graph.json must be a regular file, not a symlink or directory",
            )),
            Err(Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn check_directory(root: &File, directory: &OwnedFd) -> io::Result<()> {
        let named = fs::statat(root, DIRECTORY, AtFlags::SYMLINK_NOFOLLOW)?;
        let opened = fs::fstat(directory)?;
        if FileType::from_raw_mode(named.st_mode) != FileType::Directory
            || named.st_dev != opened.st_dev
            || named.st_ino != opened.st_ino
        {
            return Err(io::Error::other("output directory changed during save"));
        }
        Ok(())
    }

    pub(super) fn atomic_write(
        root: &File,
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> io::Result<()> {
        match fs::mkdirat(root, DIRECTORY, Mode::RWXU) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
        // All later operations use the opened directory, never re-follow a path symlink.
        let directory = fs::openat(root, DIRECTORY, DIR_FLAGS, Mode::empty())?;
        check_directory(root, &directory)?;
        check_target(&directory)?;
        let mut temporary = None;
        for _ in 0..16 {
            let name = format!(
                ".graph-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            match fs::openat(
                &directory,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => {
                    temporary = Some((name, File::from(fd)));
                    break;
                }
                Err(Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let (name, mut file) =
            temporary.ok_or_else(|| io::Error::other("temporary output names exhausted"))?;
        let result = (|| {
            check_directory(root, &directory)?;
            write(&mut file)?;
            file.sync_all()?;
            check_directory(root, &directory)?;
            check_target(&directory)?;
            let named = fs::statat(&directory, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)?;
            let opened = fs::fstat(&file)?;
            if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile
                || named.st_dev != opened.st_dev
                || named.st_ino != opened.st_ino
            {
                return Err(io::Error::other("temporary output changed during save"));
            }
            // rename replaces a target symlink itself; it never writes to its referent.
            fs::renameat(&directory, name.as_str(), &directory, TARGET)?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Err(cleanup) = fs::unlinkat(&directory, name.as_str(), AtFlags::empty()) {
                return Err(io::Error::other(format!(
                    "{error}; temporary file cleanup failed: {cleanup}"
                )));
            }
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "output_tests.rs"]
mod tests;
