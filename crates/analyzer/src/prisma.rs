//! A bounded schema recognizer, not a Prisma validator or database introspector.
use crate::{
    Confidence, Diagnostic, Evidence, EvidenceSource, GraphBuilder, GraphNode, NodeKind, Severity,
    canonical_id,
    discovery::{RepositoryRoot, discover_prisma, logical_excluded},
};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct PrismaFindings {
    pub candidates: Vec<String>,
    pub selected_schema: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub diagnostics: Vec<Diagnostic>,
}
impl PrismaFindings {
    /// `owned_discovery_diagnostics` are applied by another component (e.g. Resolver).
    /// Only identical shared discovery findings are omitted; Prisma diagnostics remain owned here.
    pub fn apply(
        &self,
        builder: &mut GraphBuilder,
        owned_discovery_diagnostics: &[Diagnostic],
    ) -> Result<(), String> {
        for node in &self.nodes {
            builder.add_node(node.clone())?;
        }
        for diagnostic in &self.diagnostics {
            if diagnostic.code.starts_with("DISCOVERY_")
                && owned_discovery_diagnostics.contains(diagnostic)
            {
                continue;
            }
            builder.add_diagnostic(diagnostic.clone())?;
        }
        Ok(())
    }
}

pub fn analyze(root: &RepositoryRoot) -> Result<PrismaFindings, String> {
    let (candidates, diagnostics) = discover_prisma(root)?;
    let selected = candidates
        .iter()
        .find(|p| p.as_str() == "prisma/schema.prisma")
        .or_else(|| candidates.first())
        .cloned();
    let mut findings = PrismaFindings {
        candidates,
        selected_schema: selected.clone(),
        nodes: vec![],
        diagnostics,
    };
    if let Some(file) = selected {
        if findings.candidates.len() > 1 {
            findings.diagnostics.push(diagnostic(
                &file,
                None,
                "PRISMA_MULTIPLE_SCHEMAS",
                "Selected this schema; additional canonical schema candidates were not analyzed",
            ));
        }
        let source = read_schema(root, &file)?;
        let parsed = parse_schema(&file, &source)?;
        findings.nodes = parsed.nodes;
        findings.diagnostics.extend(parsed.diagnostics);
    }
    Ok(findings)
}

fn read_schema(root: &RepositoryRoot, file: &str) -> Result<String, String> {
    let path = root
        .resolve(file)
        .map_err(|_| "Prisma schema path cannot be safely resolved")?;
    let relative = root
        .relative_path(&path)
        .map_err(|_| "Prisma schema path is unsafe")?;
    if relative != file || logical_excluded(Path::new(&relative)) {
        return Err("Prisma schema path changed or is excluded".into());
    }
    // Reject final-component symlink swaps and special files before reading. Ancestor
    // renames by the same user are not an atomic repository snapshot guarantee.
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
        );
    }
    let mut input = options
        .open(&path)
        .map_err(|_| "Prisma schema cannot be opened safely")?;
    if !input
        .metadata()
        .map_err(|_| "Prisma schema cannot be inspected")?
        .is_file()
    {
        return Err("Prisma schema is not a regular file".into());
    }
    let mut bytes = Vec::new();
    input
        .read_to_end(&mut bytes)
        .map_err(|_| "Prisma schema cannot be read")?;
    String::from_utf8(bytes).map_err(|_| "Prisma schema content is not valid UTF-8".into())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Ident,
    String,
    Symbol(u8),
}
#[derive(Clone, Copy, Debug)]
struct Token<'a> {
    kind: Kind,
    text: &'a str,
    line: u64,
    start: usize,
    end: usize,
}
impl Token<'_> {
    fn symbol(self, ch: u8) -> bool {
        self.kind == Kind::Symbol(ch)
    }
}
fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}
fn ident_continue(b: u8) -> bool {
    ident_start(b) || b.is_ascii_digit()
}

/// Tokens retain UTF-8 byte offsets into the unchanged source. Strings are opaque.
fn lex(source: &str) -> (Vec<Token<'_>>, Option<(u64, &'static str)>) {
    let b = source.as_bytes();
    let (mut i, mut line) = (0, 1);
    let mut tokens = Vec::new();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            if b[i] == b'\n' {
                line += 1;
            }
            i += 1;
            continue;
        }
        let (start, start_line) = (i, line);
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                if b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if i == b.len() {
                return (
                    tokens,
                    Some((
                        start_line,
                        "Unclosed block comment; remaining source was not analyzed",
                    )),
                );
            }
            i += 2;
            continue;
        }
        let kind = if b[i] == b'"' {
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'"' {
                    i += 1;
                    closed = true;
                    break;
                }
                if b[i] == b'\\' {
                    i += 1;
                    if i == b.len() {
                        break;
                    }
                }
                if b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if !closed {
                return (
                    tokens,
                    Some((
                        start_line,
                        "Unclosed string; remaining source was not analyzed",
                    )),
                );
            }
            Kind::String
        } else if ident_start(b[i]) {
            i += 1;
            while i < b.len() && ident_continue(b[i]) {
                i += 1;
            }
            Kind::Ident
        } else {
            // Advance a complete scalar, including unsupported non-ASCII syntax.
            i += source[i..].chars().next().unwrap().len_utf8();
            Kind::Symbol(b[start])
        };
        tokens.push(Token {
            kind,
            text: &source[start..i],
            line: start_line,
            start,
            end: i,
        });
    }
    (tokens, None)
}

fn diagnostic(file: &str, line: Option<u64>, code: &str, message: &str) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        severity: Severity::Warning,
        message: message.into(),
        file: Some(file.into()),
        line,
        related_node_id: None,
        skipped_count: None,
    }
}

/// Pure analysis of an already acquired source. This does not validate Prisma semantics.
pub fn parse_schema(file: &str, source: &str) -> Result<PrismaFindings, String> {
    if !crate::is_repository_path(file) {
        return Err("Prisma schema requires a repository-relative path".into());
    }
    let (tokens, lexical_error) = lex(source);
    let mut out = PrismaFindings {
        candidates: vec![file.into()],
        selected_schema: Some(file.into()),
        nodes: vec![],
        diagnostics: vec![],
    };
    let mut declarations = BTreeMap::<&str, Vec<u64>>::new();
    let mut i = 0;
    'blocks: while i < tokens.len() {
        let start = i;
        let is_model = tokens[i].kind == Kind::Ident && tokens[i].text == "model";
        let name = tokens.get(i + 1).filter(|t| t.kind == Kind::Ident);
        if is_model && let Some(name) = name {
            declarations
                .entry(name.text)
                .or_default()
                .push(tokens[i].line);
        }
        let header_valid = tokens[i].kind == Kind::Ident
            && name.is_some()
            && tokens.get(i + 2).is_some_and(|t| t.symbol(b'{'));
        // An invalid header can only be recovered at a newline or its own closed block.
        while i < tokens.len() && !tokens[i].symbol(b'{') {
            i += 1;
            if i < tokens.len() && tokens[i].line > tokens[start].line {
                break;
            }
        }
        if i == tokens.len() || !tokens[i].symbol(b'{') {
            out.diagnostics.push(diagnostic(
                file,
                Some(tokens[start].line),
                "PRISMA_UNSUPPORTED_TOP_LEVEL",
                "Top-level declaration was not recognized",
            ));
            continue;
        }
        let open = i;
        let mut delimiters = vec![b'}'];
        i += 1;
        while i < tokens.len() {
            match tokens[i].kind {
                Kind::Symbol(b'{') => delimiters.push(b'}'),
                Kind::Symbol(b'(') => delimiters.push(b')'),
                Kind::Symbol(b'[') => delimiters.push(b']'),
                Kind::Symbol(ch @ (b'}' | b')' | b']')) => {
                    if delimiters.pop() != Some(ch) {
                        out.diagnostics.push(diagnostic(file, Some(tokens[i].line), "PRISMA_BLOCK_BOUNDARY",
                            "Mismatched delimiter; this block and remaining source were not extracted"));
                        break 'blocks;
                    }
                    if delimiters.is_empty() {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i == tokens.len() {
            out.diagnostics.push(diagnostic(
                file,
                Some(tokens[start].line),
                "PRISMA_UNCLOSED_BLOCK",
                "Unclosed block; this block and remaining source were not extracted",
            ));
            break;
        }
        let close = i;
        i += 1;
        if !header_valid {
            out.diagnostics.push(diagnostic(
                file,
                Some(tokens[start].line),
                "PRISMA_INVALID_BLOCK",
                "Block header is uncertain; no model was extracted",
            ));
            continue;
        }
        if !is_model {
            continue;
        }
        let name = name.unwrap().text;
        let fields = fields(file, &tokens[open + 1..close], &mut out.diagnostics);
        let line = Some(tokens[start].line);
        let end_line = Some(tokens[close].line);
        out.nodes.push(GraphNode {
            id: canonical_id("database_model", &[file, name]),
            kind: NodeKind::DatabaseModel,
            name: name.into(),
            qualified_name: None,
            file: Some(file.into()),
            line,
            end_line,
            parent_id: None,
            evidence: vec![Evidence {
                source: EvidenceSource::Prisma,
                confidence: Confidence::Confirmed,
                file: file.into(),
                line,
                end_line,
            }],
            metadata: Some(serde_json::Map::from_iter([(
                "fields".into(),
                serde_json::Value::Array(fields),
            )])),
        });
    }
    for (name, lines) in declarations {
        if lines.len() > 1 {
            out.nodes.retain(|n| n.name != name);
            for line in lines {
                out.diagnostics.push(diagnostic(
                    file,
                    Some(line),
                    "PRISMA_DUPLICATE_MODEL",
                    "Duplicate model name; all declarations of this name were suppressed",
                ));
            }
        }
    }
    if let Some((line, message)) = lexical_error {
        out.diagnostics.push(diagnostic(
            file,
            Some(line),
            "PRISMA_LEXICAL_ERROR",
            message,
        ));
    }
    Ok(out)
}

fn fields(
    file: &str,
    tokens: &[Token<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<serde_json::Value> {
    let mut output = Vec::new();
    let mut names = BTreeMap::<&str, Vec<u64>>::new();
    let mut i = 0;
    while i < tokens.len() {
        let start = i;
        if tokens[start].kind == Kind::Ident {
            names
                .entry(tokens[start].text)
                .or_default()
                .push(tokens[start].line);
        }
        let mut depth = 0;
        while i < tokens.len() {
            let t = tokens[i];
            match t.kind {
                Kind::Symbol(b'(' | b'[' | b'{') => depth += 1,
                Kind::Symbol(b')' | b']' | b'}') => depth -= 1,
                _ => {}
            }
            i += 1;
            // The enclosing block already validated delimiter pairing.
            if depth == 0 && (i == tokens.len() || tokens[i].line > t.line) {
                break;
            }
        }
        let row = &tokens[start..i];
        let block_attribute = row[0].symbol(b'@') && row.get(1).is_some_and(|t| t.symbol(b'@'));
        if block_attribute && attributes(&row[2..], true) {
            continue;
        }
        let field = parse_field(row);
        if let Some((name, ty)) = field {
            output.push(serde_json::json!({"name":name,"type":ty}));
        } else {
            diagnostics.push(diagnostic(
                file,
                Some(row[0].line),
                "PRISMA_UNSUPPORTED_FIELD",
                "Unsupported field or attribute syntax; this declaration was not extracted",
            ));
        }
    }
    remove_duplicate_fields(output, names, file, diagnostics)
}
fn remove_duplicate_fields(
    mut fields: Vec<serde_json::Value>,
    names: BTreeMap<&str, Vec<u64>>,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<serde_json::Value> {
    for (name, lines) in names {
        if lines.len() > 1 {
            fields.retain(|f| f["name"] != name);
            for line in lines {
                diagnostics.push(diagnostic(
                    file,
                    Some(line),
                    "PRISMA_DUPLICATE_FIELD",
                    "Duplicate field name; all fields of this name were suppressed",
                ));
            }
        }
    }
    fields
}
fn parse_field<'a>(row: &[Token<'a>]) -> Option<(&'a str, String)> {
    if row.len() < 2 || row[0].kind != Kind::Ident || row[1].kind != Kind::Ident {
        return None;
    }
    let mut ty = row[1].text.to_owned();
    let mut i = 2;
    if row.get(i).is_some_and(|t| t.symbol(b'?')) {
        ty.push('?');
        i += 1;
    } else if row.get(i).is_some_and(|t| t.symbol(b'['))
        && row.get(i + 1).is_some_and(|t| t.symbol(b']'))
    {
        ty.push_str("[]");
        i += 2;
    }
    // Type suffix spelling is retained only when adjacent, not guessed across gaps.
    if i > 2 && row[1..i].windows(2).any(|t| t[0].end != t[1].start) {
        return None;
    }
    if !attributes(&row[i..], false) {
        return None;
    }
    Some((row[0].text, ty))
}
fn attributes(tokens: &[Token<'_>], first_name: bool) -> bool {
    let mut i = 0;
    let mut bare_name = first_name;
    while i < tokens.len() {
        if !bare_name {
            if !tokens[i].symbol(b'@') {
                return false;
            }
            i += 1;
        }
        bare_name = false;
        if !tokens.get(i).is_some_and(|t| t.kind == Kind::Ident) {
            return false;
        }
        i += 1;
        while tokens.get(i).is_some_and(|t| t.symbol(b'.')) {
            i += 1;
            if !tokens.get(i).is_some_and(|t| t.kind == Kind::Ident) {
                return false;
            }
            i += 1;
        }
        if tokens.get(i).is_some_and(|t| t.symbol(b'(')) {
            let mut depth = 1;
            i += 1;
            while i < tokens.len() && depth > 0 {
                if tokens[i].symbol(b'(') {
                    depth += 1;
                }
                if tokens[i].symbol(b')') {
                    depth -= 1;
                }
                i += 1;
            }
            if depth != 0 {
                return false;
            }
        }
    }
    !bare_name
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn read_boundary_rechecks_candidate_before_acquiring_bytes() {
        let path = std::env::temp_dir().join(format!(
            "canvas-prisma-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        let root = RepositoryRoot::open(&path).unwrap();
        fs::write(path.join("schema.prisma"), "model A { id Int }").unwrap();
        let (candidates, _) = discover_prisma(&root).unwrap();
        assert_eq!(candidates, ["schema.prisma"]);
        fs::remove_file(path.join("schema.prisma")).unwrap();
        assert!(
            read_schema(&root, &candidates[0])
                .unwrap_err()
                .contains("resolved")
        );
        symlink("/dev/zero", path.join("schema.prisma")).unwrap();
        assert!(read_schema(&root, &candidates[0]).is_err());
        fs::remove_file(path.join("schema.prisma")).unwrap();
        fs::create_dir(path.join("node_modules")).unwrap();
        fs::write(path.join("node_modules/schema.prisma"), "SECRET").unwrap();
        symlink("node_modules/schema.prisma", path.join("schema.prisma")).unwrap();
        assert!(read_schema(&root, &candidates[0]).is_err());
        assert!(read_schema(&root, "node_modules/schema.prisma").is_err());
        fs::remove_file(path.join("schema.prisma")).unwrap();
        fs::create_dir(path.join("schema.prisma")).unwrap();
        assert!(read_schema(&root, &candidates[0]).is_err());
        fs::remove_dir(path.join("schema.prisma")).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(path.join("schema.prisma"))
                .status()
                .unwrap()
                .success()
        );
        assert!(
            read_schema(&root, &candidates[0])
                .unwrap_err()
                .contains("regular file")
        );
        fs::remove_file(path.join("schema.prisma")).unwrap();
        fs::write(path.join("schema.prisma"), "SECRET").unwrap();
        fs::set_permissions(path.join("schema.prisma"), fs::Permissions::from_mode(0o0)).unwrap();
        // Run as a regular user: privileged users can read mode-000 files.
        if fs::File::open(path.join("schema.prisma")).is_err() {
            let error = read_schema(&root, &candidates[0]).unwrap_err();
            assert!(!error.contains("SECRET"));
            assert!(!error.contains(path.to_str().unwrap()));
        }
        fs::set_permissions(
            path.join("schema.prisma"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        fs::remove_dir_all(path).unwrap();
    }
}
