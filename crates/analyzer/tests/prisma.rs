use codebasecanvas_analyzer::{NodeKind, discovery::RepositoryRoot, prisma};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Repo(PathBuf);
impl Repo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "canvas-prisma-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn root(&self) -> RepositoryRoot {
        RepositoryRoot::open(&self.0).unwrap()
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn fixture_fields_and_positions() {
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let f = prisma::analyze(&root).unwrap();
    assert_eq!(f.selected_schema.as_deref(), Some("prisma/schema.prisma"));
    assert_eq!(
        f.nodes
            .iter()
            .map(|n| (n.name.as_str(), n.line, n.end_line))
            .collect::<Vec<_>>(),
        [("User", Some(2), Some(5)), ("Token", Some(7), Some(10))]
    );
    assert!(f.nodes.iter().all(|n| n.kind == NodeKind::DatabaseModel));
    assert_eq!(
        f.nodes[0].metadata.as_ref().unwrap()["fields"],
        serde_json::json!([{"name":"id","type":"String"},{"name":"name","type":"String"}])
    );
}
#[test]
fn empty_and_selection() {
    let r = Repo::new();
    assert!(prisma::analyze(&r.root()).unwrap().nodes.is_empty());
    r.write("a/schema.prisma", "model Other { id Int }");
    r.write("prisma/schema.prisma", "model User { id String }");
    let f = prisma::analyze(&r.root()).unwrap();
    assert_eq!(f.nodes[0].name, "User");
    assert_eq!(f.candidates.len(), 2);
    assert!(
        f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_MULTIPLE_SCHEMAS")
    );
}

fn parse(source: &str) -> prisma::PrismaFindings {
    prisma::parse_schema("prisma/schema.prisma", source).unwrap()
}
fn names(f: &prisma::PrismaFindings) -> Vec<&str> {
    f.nodes.iter().map(|n| n.name.as_str()).collect()
}
fn fields(f: &prisma::PrismaFindings, index: usize) -> serde_json::Value {
    f.nodes[index].metadata.as_ref().unwrap()["fields"].clone()
}
fn has(f: &prisma::PrismaFindings, code: &str) -> bool {
    f.diagnostics.iter().any(|d| d.code == code)
}

#[test]
fn lexical_states_attributes_and_crlf_preserve_source_positions() {
    let source = r#"// model Fake { x Int }
/// 日本語 model Fake2 { }
/* model Fake3 { } */
datasource db {
 url = "https://example.test/モデル//{model Fake4 {}}\""
}
enum Role { USER }
type Composite { x String }
generator client { provider = "model Fake5 { }" }
model User {
 id String @id @default("日本語 } // \" model Fake6 { }")
 names String[] @relation(fields: [id], references: [id])
 name String? @map("name")
 value Int @default(
   call(["}", "//", "model Fake7 { }"], nested(1))
 )
 @@map("users")
}
model Token { id Int }
"#;
    let f = parse(source);
    assert_eq!(names(&f), ["User", "Token"]);
    assert!(f.diagnostics.is_empty(), "{:?}", f.diagnostics);
    assert_eq!(
        fields(&f, 0),
        serde_json::json!([{"name":"id","type":"String"},{"name":"names","type":"String[]"},{"name":"name","type":"String?"},{"name":"value","type":"Int"}])
    );
    assert_eq!((f.nodes[0].line, f.nodes[0].end_line), (Some(10), Some(18)));
    assert_eq!(parse(&source.replace('\n', "\r\n")), f);
}
#[test]
fn no_models_and_non_model_blocks() {
    for source in [
        "",
        "// hello",
        "generator x { model Fake { id Int } }",
        "view V { id Int }",
        "enum E { A }",
        "datasource x { url = \"model Fake {}\" }",
    ] {
        let f = parse(source);
        assert!(f.nodes.is_empty());
        assert!(f.diagnostics.is_empty());
    }
}
#[test]
fn malformed_fields_are_local_and_duplicates_are_suppressed() {
    let f = parse(
        "model A {\n id Int\n bad Unsupported(\"X\")\n good String?\n dup Int\n dup String\n}\nmodel B { ok A[] }\nmodel A { x Int }",
    );
    assert_eq!(names(&f), ["B"]);
    assert!(has(&f, "PRISMA_DUPLICATE_MODEL"));
    assert!(has(&f, "PRISMA_DUPLICATE_FIELD"));
    assert!(has(&f, "PRISMA_UNSUPPORTED_FIELD"));
    let f = parse("model A {\n id Int\n broken What(\"secret\")\n good String\n}");
    assert_eq!(
        fields(&f, 0),
        serde_json::json!([{"name":"id","type":"Int"},{"name":"good","type":"String"}])
    );
    assert!(f.diagnostics.iter().all(|d| d.related_node_id.is_none()
        && d.skipped_count.is_none()
        && !d.message.contains("secret")));
    let f = parse("model A {\n x Int\n x String\n y Int\n}");
    assert_eq!(
        fields(&f, 0),
        serde_json::json!([{"name":"y","type":"Int"}])
    );
}
#[test]
fn uncertain_lexical_or_block_boundaries_never_resume_inside_text() {
    for tail in [
        "model Bad {\n x Int",
        "model Bad { x String @default(\"model Fake {}",
        "/* model Fake {}",
        "model Bad { x String /* model Fake {}",
    ] {
        let f = parse(&format!("model Good {{ id Int }}\n{tail}"));
        assert_eq!(names(&f), ["Good"]);
        assert!(!f.diagnostics.is_empty());
    }
    let f = parse("model { x Int }\nmodel Good { id Int }");
    assert_eq!(names(&f), ["Good"]);
    assert!(has(&f, "PRISMA_INVALID_BLOCK"));
    let f = parse("model A { id Int }\nmodel A { id Int");
    assert!(f.nodes.is_empty());
    assert!(has(&f, "PRISMA_DUPLICATE_MODEL"));
}
#[test]
fn uncertain_field_delimiters_suppress_uncertain_block_and_remainder() {
    for bad in [
        "bad Int @default(]\n next String",
        "bad Int @default(\n next String",
    ] {
        let f = parse(&format!(
            "model A {{\n id Int\n{bad}\n}}\nmodel B {{ x Int }}"
        ));
        assert!(f.nodes.is_empty());
        assert!(has(&f, "PRISMA_BLOCK_BOUNDARY"));
    }
}
#[test]
fn deterministic_candidates_and_no_fallback_from_bad_preferred_schema() {
    let a = Repo::new();
    let b = Repo::new();
    for (repo, paths) in [
        (&a, ["z/schema.prisma", "a/schema.prisma"]),
        (&b, ["a/schema.prisma", "z/schema.prisma"]),
    ] {
        for path in paths {
            repo.write(path, "model A { id Int }");
        }
    }
    assert_eq!(
        prisma::analyze(&a.root()).unwrap(),
        prisma::analyze(&b.root()).unwrap()
    );
    assert_eq!(
        prisma::analyze(&a.root())
            .unwrap()
            .selected_schema
            .as_deref(),
        Some("a/schema.prisma")
    );
    a.write("prisma/schema.prisma", "model Broken {");
    let f = prisma::analyze(&a.root()).unwrap();
    assert!(f.nodes.is_empty());
    assert!(has(&f, "PRISMA_UNCLOSED_BLOCK"));
    assert_eq!(f.selected_schema.as_deref(), Some("prisma/schema.prisma"));
}
#[cfg(unix)]
#[test]
fn shared_discovery_aliases_exclusions_loops_and_external_boundary() {
    use std::os::unix::fs::symlink;
    let r = Repo::new();
    r.write("real/schema.prisma", "model A { id Int }");
    r.write("main.ts", "class A {}");
    r.write("node_modules/pkg/schema.prisma", "model Hidden { id Int }");
    symlink("real", r.0.join("a-alias")).unwrap();
    symlink("real/schema.prisma", r.0.join("schema.prisma")).unwrap();
    symlink(".", r.0.join("loop")).unwrap();
    symlink("node_modules", r.0.join("vendor")).unwrap();
    let f = prisma::analyze(&r.root()).unwrap();
    assert_eq!(f.candidates, ["real/schema.prisma"]);
    assert!(!has(&f, "PRISMA_MULTIPLE_SCHEMAS"));
    assert!(has(&f, "DISCOVERY_SYMLINK_LOOP"));
    assert!(has(&f, "DISCOVERY_EXCLUDED_ALIAS"));
    let ts = codebasecanvas_analyzer::discovery::discover(&r.root()).unwrap();
    assert_eq!(ts.files, ["main.ts"]);
    let outside = Repo::new();
    outside.write("schema.prisma", "SECRET");
    symlink(&outside.0, r.0.join("external")).unwrap();
    let error = prisma::analyze(&r.root()).unwrap_err();
    assert!(!error.contains("SECRET"));
    assert!(!error.contains(outside.0.to_str().unwrap()));
}
#[cfg(unix)]
#[test]
fn unresolved_links_and_invalid_utf8_are_distinct() {
    use std::os::unix::fs::symlink;
    let r = Repo::new();
    symlink("missing", r.0.join("schema.prisma")).unwrap();
    let f = prisma::analyze(&r.root()).unwrap();
    assert!(f.nodes.is_empty());
    assert!(has(&f, "DISCOVERY_UNRESOLVED_SYMLINK"));
    fs::remove_file(r.0.join("schema.prisma")).unwrap();
    fs::write(r.0.join("schema.prisma"), [0xff]).unwrap();
    assert!(
        prisma::analyze(&r.root())
            .unwrap_err()
            .contains("content is not valid UTF-8")
    );
    fs::remove_file(r.0.join("schema.prisma")).unwrap();
    // macOS rejects invalid UTF-8 filenames at creation; Linux exercises the walk.
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        fs::write(r.0.join(OsString::from_vec(b"bad\xff".to_vec())), "").unwrap();
        assert!(
            prisma::analyze(&r.root())
                .unwrap_err()
                .contains("path is not valid UTF-8")
        );
    }
}
fn builder() -> codebasecanvas_analyzer::GraphBuilder {
    use codebasecanvas_analyzer::*;
    GraphBuilder::new(GraphMetadata {
        analyzer_version: "prisma-test".into(),
        analyzed_at: "2026-09-22T00:00:00Z".into(),
        root_name: None,
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    })
}
#[test]
fn findings_apply_without_rereading_and_identity_depends_on_file() {
    let r = Repo::new();
    r.write("schema.prisma", "model A { id Int }");
    let f = prisma::analyze(&r.root()).unwrap();
    r.write("schema.prisma", "model Changed { id String }");
    let mut b = builder();
    f.apply(&mut b, &[]).unwrap();
    f.apply(&mut b, &[]).unwrap();
    let g = b.finish().unwrap();
    assert_eq!(g.nodes.len(), 1);
    assert_eq!(g.nodes[0].name, "A");
    assert_eq!(g.nodes[0].evidence.len(), 1);
    assert!(g.edges.is_empty());
    let other = prisma::parse_schema("other/schema.prisma", "model A { id Int }").unwrap();
    assert_ne!(f.nodes[0].id, other.nodes[0].id);
    assert!(prisma::parse_schema("/absolute/schema.prisma", "").is_err());
}
#[test]
fn fixture_oracle_projection_and_calls_are_unchanged() {
    use codebasecanvas_analyzer::{
        EdgeKind, SystemGraph, calls, nestjs_di, nestjs_modules, nestjs_roles, nestjs_routes,
        resolver::ImportResolver,
    };
    let root = RepositoryRoot::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/nestjs-sample"),
    )
    .unwrap();
    let r = ImportResolver::analyze(&root).unwrap();
    let make = || {
        let mut b = builder();
        for (file, _) in r.sources() {
            nestjs_roles::extract_file(file, &r, &mut b).unwrap();
        }
        nestjs_modules::analyze(&r, &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
        nestjs_routes::analyze(&r, &b)
            .unwrap()
            .apply(&mut b)
            .unwrap();
        nestjs_di::analyze(&r, &b).unwrap().apply(&mut b).unwrap();
        calls::analyze(&r, &b).unwrap().apply(&mut b).unwrap();
        r.apply_imports(&mut b).unwrap();
        b
    };
    let before = make().finish().unwrap();
    let mut b = make();
    prisma::analyze(&root)
        .unwrap()
        .apply(&mut b, r.diagnostics())
        .unwrap();
    let after = b.finish().unwrap();
    assert_eq!(
        before.nodes,
        after
            .nodes
            .iter()
            .filter(|n| n.kind != NodeKind::DatabaseModel)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(before.edges, after.edges);
    assert_eq!(before.diagnostics, after.diagnostics);
    assert_eq!(before.metadata, after.metadata);
    let c = &after.metadata.call_analysis;
    assert_eq!(
        (c.examined_calls, c.emitted_calls, c.skipped_calls),
        (9, 2, 7)
    );
    assert_eq!(
        after
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count(),
        2
    );
    let oracle = SystemGraph::from_json(include_str!(
        "../../../examples/nestjs-sample/expected-graph.json"
    ))
    .unwrap();
    let projection = |g: &SystemGraph| {
        g.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::DatabaseModel)
            .map(|n| {
                assert!(n.parent_id.is_none());
                (
                    n.id.clone(),
                    n.kind,
                    n.name.clone(),
                    n.file.clone(),
                    n.line,
                    n.evidence
                        .iter()
                        .map(|e| (e.source, e.file.clone(), e.line, e.confidence))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(projection(&after), projection(&oracle));
    for (name, end, expected_fields) in [
        (
            "User",
            5,
            serde_json::json!([{"name":"id","type":"String"},{"name":"name","type":"String"}]),
        ),
        (
            "Token",
            10,
            serde_json::json!([{"name":"id","type":"String"},{"name":"value","type":"String"}]),
        ),
    ] {
        let n = after
            .nodes
            .iter()
            .find(|n| n.name == name && n.kind == NodeKind::DatabaseModel)
            .unwrap();
        assert_eq!(n.end_line, Some(end));
        assert_eq!(n.evidence[0].end_line, Some(end));
        assert_eq!(n.metadata.as_ref().unwrap()["fields"], expected_fields);
    }
}

#[test]
fn mismatched_delimiters_do_not_promote_remainder_to_top_level() {
    for block in ["model A", "generator client", "datasource db"] {
        let source = format!(
            "model Good {{ id Int }}\n{block} {{ bad Int @default(}}\nmodel Fake {{ id Int }}"
        );
        let f = parse(&source);
        assert_eq!(names(&f), ["Good"]);
        assert!(has(&f, "PRISMA_BLOCK_BOUNDARY"));
    }
}

#[cfg(unix)]
#[test]
fn resolver_owns_shared_discovery_diagnostics_but_not_prisma_diagnostics() {
    use codebasecanvas_analyzer::resolver::ImportResolver;
    use std::os::unix::fs::symlink;
    let repo = Repo::new();
    repo.write(
        "real/schema.prisma",
        "model A {\n bad Unsupported(\"X\")\n}",
    );
    symlink("real", repo.0.join("alias")).unwrap();
    let r = ImportResolver::analyze(&repo.root()).unwrap();
    let f = prisma::analyze(&repo.root()).unwrap();
    assert_eq!(
        f.diagnostics
            .iter()
            .filter(|d| d.code == "DISCOVERY_SYMLINK_LOOP")
            .count(),
        1
    );
    let mut b = builder();
    r.apply_imports(&mut b).unwrap();
    f.apply(&mut b, r.diagnostics()).unwrap();
    let graph = b.finish().unwrap();
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter(|d| d.code == "DISCOVERY_SYMLINK_LOOP")
            .count(),
        1
    );
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter(|d| d.code == "PRISMA_UNSUPPORTED_FIELD")
            .count(),
        1
    );
    let mut b = builder();
    f.apply(&mut b, &[]).unwrap();
    f.apply(&mut b, &[]).unwrap();
    let graph = b.finish().unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter(|d| d.code == "PRISMA_UNSUPPORTED_FIELD")
            .count(),
        2
    );
}

#[test]
fn multiline_model_header_is_not_confirmed() {
    for source in [
        "model User\n{ id Int }",
        "model\nUser { id Int }",
        "model User /* a\n b */ { id Int }",
    ] {
        for text in [source.to_owned(), source.replace('\n', "\r\n")] {
            let f = parse(&text);
            assert!(f.nodes.is_empty(), "{text:?}");
            assert!(
                f.diagnostics
                    .iter()
                    .any(|d| d.code == "PRISMA_INVALID_BLOCK"
                        && d.file.as_deref() == Some("prisma/schema.prisma")
                        && d.line == Some(1)
                        && d.related_node_id.is_none()),
                "{text:?}: {:?}",
                f.diagnostics
            );
        }
    }

    let f = parse("model Before { id Int }\nmodel User\n{ id Int }\nmodel After { id Int }");
    assert_eq!(names(&f), ["Before", "After"]);
    assert_eq!(
        f.diagnostics
            .iter()
            .filter(|d| d.code == "PRISMA_INVALID_BLOCK" && d.line == Some(2))
            .count(),
        1
    );

    let f = parse("model /* same line */ User { id Int }");
    assert_eq!(names(&f), ["User"]);
    assert!(f.diagnostics.is_empty());

    let f = parse("model User\n{ id Int }\nmodel User { id Int }");
    assert!(f.nodes.is_empty());
    assert!(
        f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_DUPLICATE_MODEL")
    );
}

#[test]
fn unknown_top_level_block_is_diagnosed() {
    let f = parse("modle User { id Int }");
    assert!(f.nodes.is_empty());
    assert!(
        f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_UNSUPPORTED_TOP_LEVEL"
                && d.file.as_deref() == Some("prisma/schema.prisma")
                && d.line == Some(1)
                && d.related_node_id.is_none())
    );

    for keyword in ["modle", "models", "Model", "custom"] {
        let source = format!(
            "// {keyword} Hidden {{ id Int }}\nmodel Before {{ id Int }}\n{keyword} Unknown {{\n model Fake {{ id Int }}\n}}\nmodel After {{ id Int }}"
        );
        let f = parse(&source);
        assert_eq!(names(&f), ["Before", "After"]);
        assert_eq!(
            f.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            [
                codebasecanvas_analyzer::canonical_id(
                    "database_model",
                    &["prisma/schema.prisma", "Before"]
                ),
                codebasecanvas_analyzer::canonical_id(
                    "database_model",
                    &["prisma/schema.prisma", "After"]
                ),
            ]
        );
        assert_eq!(f.diagnostics.len(), 1);
        let d = &f.diagnostics[0];
        assert_eq!(
            (
                d.code.as_str(),
                d.file.as_deref(),
                d.line,
                d.related_node_id.as_deref()
            ),
            (
                "PRISMA_UNSUPPORTED_TOP_LEVEL",
                Some("prisma/schema.prisma"),
                Some(3),
                None
            )
        );
    }

    for keyword in ["generator", "datasource", "enum", "type", "view"] {
        let f = parse(&format!("{keyword} Known {{ model Fake {{ id Int }} }}"));
        assert!(f.nodes.is_empty());
        assert!(f.diagnostics.is_empty());
    }

    let f = parse("generator\nclient { provider = \"x\" }");
    assert!(f.nodes.is_empty());
    assert!(
        f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_INVALID_BLOCK" && d.line == Some(1))
    );

    let f = parse(
        "model Before { id Int }\nmodle Unknown { value Int @default(}\nmodel Fake { id Int }",
    );
    assert_eq!(names(&f), ["Before"]);
    assert!(
        f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_BLOCK_BOUNDARY" && d.line == Some(2))
    );
    assert!(
        !f.diagnostics
            .iter()
            .any(|d| d.code == "PRISMA_UNSUPPORTED_TOP_LEVEL")
    );
}
