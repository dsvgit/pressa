//! The crate boundaries of `docs/architecture.md` §2 and
//! [ADR-0006](../../docs/adr/0006-workspace-of-four-crates.md).
//!
//! The compiler enforces a boundary only once code reaches across it. This
//! test enforces it one step earlier, at the line in `Cargo.toml` that would
//! make the reach possible — the most reviewable form of the violation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CRATES: [&str; 4] = ["pressa-core", "pressa-storage", "pressa-app", "pressa-tui"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pressa-tui sits inside the workspace root")
        .to_path_buf()
}

fn manifest(krate: &str) -> String {
    let path = workspace_root().join(krate).join("Cargo.toml");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `s` without surrounding whitespace and TOML quoting.
fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"').trim_matches('\'')
}

/// The contents of the first double-quoted string in `s`.
fn first_quoted(s: &str) -> Option<&str> {
    let (_, rest) = s.split_once('"')?;
    let (value, _) = rest.split_once('"')?;
    Some(value)
}

/// The crate named by `package = "…"` inside an inline table — a renamed
/// dependency, which is what a boundary violation looks like when someone is
/// working around this test rather than tripping over it.
fn inline_package(value: &str) -> Option<&str> {
    let (_, rest) = value.split_once("package")?;
    let (_, rest) = rest.split_once('=')?;
    first_quoted(rest)
}

/// Every crate `manifest` pulls in, from any dependency table: normal, dev,
/// build and `[target.'cfg(…)'.dependencies]`, written inline or as a table.
///
/// A dependency's name is the crate it resolves to, not the key it is filed
/// under, so `db = { package = "rusqlite" }` counts as a dependency on
/// `rusqlite`.
fn declared_dependencies(manifest: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut in_dependency_table = false;
    // Set while inside `[…dependencies.<name>]`, where keys are that
    // dependency's fields rather than further dependency names.
    let mut table_of: Option<String> = None;

    for line in manifest.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_dependency_table = header.contains("dependencies");
            table_of = None;
            if in_dependency_table {
                if let Some((_, name)) = header.rsplit_once("dependencies.") {
                    let name = unquote(name);
                    found.insert(name.to_string());
                    table_of = Some(name.to_string());
                }
            }
            continue;
        }

        if !in_dependency_table || line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if table_of.is_some() {
            if key.trim() == "package" {
                if let Some(name) = first_quoted(value) {
                    found.insert(name.to_string());
                }
            }
            continue;
        }

        if let Some(name) = inline_package(value) {
            found.insert(name.to_string());
        }
        // `dep = …`, `"dep" = …` and `dep.workspace = true` all name `dep`.
        let key = unquote(key);
        let key = key.split('.').next().unwrap_or(key);
        found.insert(key.to_string());
    }

    found
}

/// Whether `manifest` pulls in `dep` under any name, in any dependency table.
fn depends_on(manifest: &str, dep: &str) -> bool {
    declared_dependencies(manifest).contains(dep)
}

fn assert_forbidden(krate: &str, forbidden: &[&str]) {
    let manifest = manifest(krate);
    for dep in forbidden {
        assert!(
            !depends_on(&manifest, dep),
            "{krate} must not depend on {dep} (docs/architecture.md §2)"
        );
    }
}

fn assert_required(krate: &str, required: &[&str]) {
    let manifest = manifest(krate);
    for dep in required {
        assert!(
            depends_on(&manifest, dep),
            "{krate} must depend on {dep} (docs/architecture.md §2)"
        );
    }
}

#[test]
fn the_workspace_has_exactly_the_four_crates_of_adr_0006() {
    let root = fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root Cargo.toml");
    for krate in CRATES {
        assert!(
            root.contains(&format!("\"{krate}\"")),
            "{krate} is not a workspace member"
        );
        assert!(
            workspace_root().join(krate).join("Cargo.toml").is_file(),
            "{krate}/Cargo.toml is missing"
        );
    }
}

#[test]
fn core_knows_nothing_of_the_ui_or_the_database() {
    assert_forbidden(
        "pressa-core",
        &[
            "ratatui",
            "crossterm",
            "rusqlite",
            "clap",
            "pressa-storage",
            "pressa-app",
            "pressa-tui",
        ],
    );
}

#[test]
fn storage_knows_nothing_of_the_ui() {
    assert_forbidden(
        "pressa-storage",
        &["ratatui", "crossterm", "clap", "pressa-app", "pressa-tui"],
    );
}

#[test]
fn app_knows_nothing_of_the_ui_or_sqlite() {
    // `pressa-storage` is forbidden too: app depending on the adapter instead
    // of the port would invert the arrow in `docs/architecture.md` §1.
    assert_forbidden(
        "pressa-app",
        &[
            "ratatui",
            "crossterm",
            "rusqlite",
            "pressa-storage",
            "pressa-tui",
        ],
    );
}

#[test]
fn the_tui_never_touches_sqlite_directly() {
    assert_forbidden("pressa-tui", &["rusqlite"]);
}

#[test]
fn dependencies_point_inward() {
    assert_required("pressa-storage", &["pressa-core"]);
    assert_required("pressa-app", &["pressa-core"]);
    assert_required("pressa-tui", &["pressa-app", "pressa-storage"]);
}

#[test]
fn nothing_in_the_workspace_is_async() {
    // ADR-0002: synchronous rusqlite, no tokio.
    for krate in CRATES {
        assert_forbidden(krate, &["tokio", "async-std", "sqlx"]);
    }
}

#[test]
fn the_scanner_sees_a_dependency_however_it_is_written() {
    let shapes = [
        "[dependencies]\nrusqlite = \"0.32\"",
        "[dependencies]\nrusqlite=\"0.32\"",
        "[dependencies]\nrusqlite  =  \"0.32\"",
        "[dependencies]\n\"rusqlite\" = \"0.32\"",
        "[dependencies]\nrusqlite.workspace = true",
        "[dependencies]\ndb = { package = \"rusqlite\", version = \"0.32\" }",
        "[dev-dependencies]\nrusqlite = \"0.32\"",
        "[build-dependencies.rusqlite]\nversion = \"0.32\"",
        "[dependencies.db]\nversion = \"0.32\"\npackage = \"rusqlite\"",
        "[target.'cfg(unix)'.dependencies]\nrusqlite = \"0.32\"",
    ];
    for shape in shapes {
        assert!(depends_on(shape, "rusqlite"), "scanner missed:\n{shape}");
    }
}

#[test]
fn the_scanner_does_not_invent_dependencies() {
    let manifest = "\
[package]
name = \"pressa-tui\"
version = \"0.0.0\"

[dependencies]
# Wiring only, in the composition root.
pressa-app.workspace = true
serde = { version = \"1\", features = [\"derive\"] }
";
    assert!(depends_on(manifest, "pressa-app"));
    assert!(depends_on(manifest, "serde"));
    for absent in [
        "rusqlite",
        "pressa-tui",
        "name",
        "version",
        "workspace",
        "features",
        "derive",
    ] {
        assert!(!depends_on(manifest, absent), "scanner invented: {absent}");
    }
}
