use crate::{Diagnostic, Severity, is_repository_path};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".codebasecanvas",
    "node_modules",
    "dist",
    "build",
    "coverage",
    ".next",
    "generated",
    "__generated__",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRoot {
    path: PathBuf,
}

impl RepositoryRoot {
    pub fn open(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path).map_err(|_| "repository cannot be opened".to_owned())?;
        if !metadata.is_dir() {
            return Err("repository is not a directory".to_owned());
        }
        let path = path
            .canonicalize()
            .map_err(|_| "repository path cannot be resolved".to_owned())?;
        if !path.is_dir() {
            return Err("repository is not a directory".to_owned());
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn relative_path(&self, path: &Path) -> Result<String, String> {
        let resolved = path
            .canonicalize()
            .map_err(|_| "path cannot be resolved".to_owned())?;
        let relative = resolved
            .strip_prefix(&self.path)
            .map_err(|_| "path is outside the repository".to_owned())?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "path is not valid UTF-8".to_owned())?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if !is_repository_path(&relative) {
            return Err("path is not a valid repository-relative path".to_owned());
        }
        Ok(relative)
    }

    pub fn resolve(&self, relative: &str) -> Result<PathBuf, String> {
        if !is_repository_path(relative) {
            return Err("path is not a valid repository-relative path".to_owned());
        }
        let path = self.path.join(relative);
        let resolved = path
            .canonicalize()
            .map_err(|_| "path cannot be resolved".to_owned())?;
        if !resolved.starts_with(&self.path) {
            return Err("path is outside the repository".to_owned());
        }
        Ok(resolved)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredRepository {
    pub root: RepositoryRoot,
    pub files: Vec<String>,
    pub tsconfig: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn discover(root: &RepositoryRoot) -> Result<DiscoveredRepository, String> {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    let mut visited = HashSet::new();
    let root_path = root.path().to_path_buf();
    visited.insert(root_path.clone());
    walk(
        root,
        &root_path,
        Path::new(""),
        &mut visited,
        &mut files,
        &mut diagnostics,
    )?;
    files.sort();
    files.dedup();

    let tsconfig = discover_tsconfig(root)?;

    Ok(DiscoveredRepository {
        root: root.clone(),
        files,
        tsconfig,
        diagnostics,
    })
}

fn walk(
    root: &RepositoryRoot,
    actual: &Path,
    logical: &Path,
    visited: &mut HashSet<PathBuf>,
    files: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(actual)
        .map_err(|_| format!("cannot read repository directory {}", display(logical)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("cannot read repository directory {}", display(logical)))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let child_logical = logical.join(&name);
        name.to_str()
            .ok_or_else(|| format!("repository path is not valid UTF-8 at {}", display(logical)))?;
        if logical_excluded(&child_logical) {
            continue;
        }
        let link = fs::symlink_metadata(entry.path())
            .map_err(|_| format!("cannot inspect repository path {}", display(&child_logical)))?
            .file_type()
            .is_symlink();
        let resolved = match entry.path().canonicalize() {
            Ok(path) => path,
            Err(error) if link && is_expected_unresolved_symlink(&error) => {
                diagnostics.push(diagnostic(
                    "DISCOVERY_UNRESOLVED_SYMLINK",
                    Severity::Warning,
                    "Skipped an unresolved filesystem reference",
                    Some(&relative(&child_logical)?),
                ));
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "cannot resolve repository path {}: {error}",
                    display(&child_logical)
                ));
            }
        };
        if !resolved.starts_with(root.path()) {
            diagnostics.push(diagnostic(
                "DISCOVERY_OUTSIDE_ROOT",
                Severity::Warning,
                "Skipped a filesystem reference outside the repository root",
                Some(&relative(&child_logical)?),
            ));
            continue;
        }
        if excluded_resolved(root, &resolved) {
            if link {
                diagnostics.push(diagnostic(
                    "DISCOVERY_EXCLUDED_ALIAS",
                    Severity::Warning,
                    "Skipped a symlink alias to an excluded directory",
                    Some(&relative(&child_logical)?),
                ));
            }
            continue;
        }
        let metadata = fs::metadata(&resolved)
            .map_err(|_| format!("cannot inspect repository path {}", display(&child_logical)))?;
        if metadata.is_dir() {
            if !visited.insert(resolved.clone()) {
                diagnostics.push(diagnostic(
                    "DISCOVERY_SYMLINK_LOOP",
                    Severity::Warning,
                    "Skipped a filesystem loop or duplicate directory alias",
                    Some(&relative(&child_logical)?),
                ));
                continue;
            }
            walk(root, &resolved, &child_logical, visited, files, diagnostics)?;
        } else if metadata.is_file()
            && resolved
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_source)
        {
            files.push(root.relative_path(&resolved)?);
        }
    }
    Ok(())
}

fn discover_tsconfig(root: &RepositoryRoot) -> Result<Option<String>, String> {
    let path = root.path().join("tsconfig.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot inspect tsconfig.json: {error}")),
    };
    let resolved = root.resolve("tsconfig.json")?;
    if excluded_resolved(root, &resolved)
        || !fs::metadata(&resolved)
            .map_err(|error| format!("cannot inspect tsconfig.json: {error}"))?
            .is_file()
    {
        return Err("tsconfig.json is unsafe or not a regular file".to_owned());
    }
    if metadata.file_type().is_symlink() && !resolved.starts_with(root.path()) {
        return Err("tsconfig.json resolves outside the repository".to_owned());
    }
    Ok(Some("tsconfig.json".to_owned()))
}

fn is_expected_unresolved_symlink(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::NotFound {
        return true;
    }
    #[cfg(unix)]
    return error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error());
    #[cfg(not(unix))]
    false
}

fn is_source(name: &str) -> bool {
    (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.ends_with(".d.ts")
}

fn logical_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
    })
}

fn excluded_resolved(root: &RepositoryRoot, path: &Path) -> bool {
    root.relative_path(path)
        .ok()
        .is_some_and(|relative| logical_excluded(Path::new(&relative)))
}

fn relative(logical: &Path) -> Result<String, String> {
    let relative = logical
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_owned())?;
    if relative.is_empty() {
        return Err("path is empty".to_owned());
    }
    if !is_repository_path(relative) {
        return Err("path is not a valid repository-relative path".to_owned());
    }
    Ok(relative.replace(std::path::MAIN_SEPARATOR, "/"))
}

fn display(path: &Path) -> String {
    path.to_str()
        .unwrap_or("<non-UTF-8 path>")
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn diagnostic(code: &str, severity: Severity, message: &str, file: Option<&str>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity,
        message: message.to_owned(),
        file: file.map(str::to_owned),
        line: None,
        related_node_id: None,
        skipped_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "codebasecanvas-discovery-{label}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn discovers_sorted_sources_and_rejects_unsafe_aliases() {
        let path = temp_root("fixture");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src/z")).unwrap();
        fs::create_dir_all(path.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(path.join("dist")).unwrap();
        fs::write(path.join("src/z/z.ts"), "").unwrap();
        fs::write(path.join("src/a.ts"), "").unwrap();
        fs::write(path.join("src/types.d.ts"), "").unwrap();
        fs::write(path.join("node_modules/pkg/index.ts"), "").unwrap();
        fs::write(path.join("dist/generated.ts"), "").unwrap();
        fs::write(path.join("tsconfig.json"), "{}").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("../node_modules", path.join("src/vendor")).unwrap();
            std::os::unix::fs::symlink(".", path.join("src/loop")).unwrap();
        }

        let root = RepositoryRoot::open(&path).unwrap();
        assert!(root.resolve("../outside.ts").is_err());
        assert!(root.resolve("/outside.ts").is_err());
        let found = discover(&root).unwrap();
        assert_eq!(found.files, ["src/a.ts", "src/z/z.ts"]);
        assert_eq!(found.tsconfig.as_deref(), Some("tsconfig.json"));
        assert!(
            found
                .diagnostics
                .iter()
                .any(|d| d.code == "DISCOVERY_EXCLUDED_ALIAS")
        );
        #[cfg(unix)]
        assert!(
            found
                .diagnostics
                .iter()
                .any(|d| d.code == "DISCOVERY_SYMLINK_LOOP")
        );
        assert!(found.files.iter().all(|file| !file.starts_with('/')));
        let _ = fs::remove_dir_all(path);
    }

    #[cfg(unix)]
    #[test]
    fn skips_external_and_invalid_symlinks_without_leaking_absolute_paths() {
        let path = temp_root("boundary");
        let outside = temp_root("outside");
        let _ = fs::remove_dir_all(&path);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&path).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("outside.ts"), "").unwrap();
        std::os::unix::fs::symlink(&outside, path.join("external")).unwrap();
        std::os::unix::fs::symlink("missing.ts", path.join("missing.ts")).unwrap();

        let root = RepositoryRoot::open(&path).unwrap();
        let found = discover(&root).unwrap();
        assert!(found.files.is_empty());
        assert!(
            found
                .diagnostics
                .iter()
                .any(|d| d.code == "DISCOVERY_OUTSIDE_ROOT")
        );
        assert!(
            found
                .diagnostics
                .iter()
                .any(|d| d.code == "DISCOVERY_UNRESOLVED_SYMLINK")
        );
        assert!(
            found
                .diagnostics
                .iter()
                .all(|d| !d.message.contains(path.to_str().unwrap()))
        );
        let _ = fs::remove_dir_all(path);
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_paths_deduplicate_directory_aliases_and_use_target_extension() {
        use std::os::unix::fs::symlink;

        let path = temp_root("canonical-files");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("real")).unwrap();
        fs::write(path.join("real/included.ts"), "").unwrap();
        fs::write(path.join("real/ignored.js"), "").unwrap();
        symlink("real", path.join("a-alias")).unwrap();
        symlink("real", path.join("z-alias")).unwrap();
        symlink("real/ignored.js", path.join("ignored.ts")).unwrap();

        let root = RepositoryRoot::open(&path).unwrap();
        let found = discover(&root).unwrap();
        assert_eq!(found.files, ["real/included.ts"]);
        assert!(found.files.iter().all(|file| !file.contains("alias")));

        let _ = fs::remove_dir_all(path);
    }

    #[cfg(unix)]
    #[test]
    fn directory_alias_names_do_not_change_canonical_output() {
        use std::os::unix::fs::symlink;

        let first = temp_root("alias-order-first");
        let second = temp_root("alias-order-second");
        for path in [&first, &second] {
            let _ = fs::remove_dir_all(path);
            fs::create_dir_all(path.join("real")).unwrap();
            fs::write(path.join("real/source.ts"), "").unwrap();
        }
        symlink("real", first.join("a-alias")).unwrap();
        symlink("real", first.join("z-alias")).unwrap();
        symlink("real", second.join("m-alias")).unwrap();
        symlink("real", second.join("n-alias")).unwrap();

        let first_found = discover(&RepositoryRoot::open(&first).unwrap()).unwrap();
        let second_found = discover(&RepositoryRoot::open(&second).unwrap()).unwrap();
        assert_eq!(first_found.files, ["real/source.ts"]);
        assert_eq!(first_found.files, second_found.files);

        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_tsconfig_is_fatal_and_missing_tsconfig_is_optional() {
        use std::os::unix::fs::symlink;

        let missing = temp_root("tsconfig-missing");
        let _ = fs::remove_dir_all(&missing);
        fs::create_dir_all(&missing).unwrap();
        assert_eq!(
            discover(&RepositoryRoot::open(&missing).unwrap())
                .unwrap()
                .tsconfig,
            None
        );
        let _ = fs::remove_dir_all(&missing);

        let unsafe_config = temp_root("tsconfig-unsafe");
        let outside = temp_root("tsconfig-outside");
        let _ = fs::remove_dir_all(&unsafe_config);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&unsafe_config).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("tsconfig.json"), "{}").unwrap();
        symlink(
            outside.join("tsconfig.json"),
            unsafe_config.join("tsconfig.json"),
        )
        .unwrap();
        assert!(discover(&RepositoryRoot::open(&unsafe_config).unwrap()).is_err());
        let _ = fs::remove_dir_all(unsafe_config);
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_entry_is_not_silently_skipped() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = temp_root("non-utf8");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        let invalid_name = OsString::from_vec(b"invalid-\xff.ts".to_vec());
        let invalid = Path::new(invalid_name.as_os_str());
        assert!(relative(invalid).is_err());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn only_dangling_or_looping_symlinks_are_expected_resolution_warnings() {
        assert!(is_expected_unresolved_symlink(&std::io::Error::from(
            std::io::ErrorKind::NotFound,
        )));
        assert!(!is_expected_unresolved_symlink(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        )));
        #[cfg(unix)]
        assert!(is_expected_unresolved_symlink(
            &std::io::Error::from_raw_os_error(rustix::io::Errno::LOOP.raw_os_error(),)
        ));
    }
}
