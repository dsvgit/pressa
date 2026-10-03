//! Rules about the crate's own source text, in the style of
//! [`architecture.rs`](architecture.rs) — which checks `Cargo.toml` the same
//! way, and for the same reason: the compiler cannot state these.

use std::fs;
use std::path::{Path, PathBuf};

/// `pressa-tui/src/`, whatever directory the tests are run from.
fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    // `read_dir` yields a `Result` per entry; a test may simply fail on one.
    for entry in fs::read_dir(dir).expect("the source directory is readable") {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            // Recursion, because `src/tui/view/` is two levels down.
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}

#[test]
fn no_key_is_named_outside_the_keymap() {
    // ADR-0005: key handling lives in one table. A `KeyCode::` anywhere else is
    // the beginning of the `match key { … }` scattered across screens that the
    // decision exists to prevent.
    let keymap = source_root().join("tui").join("keymap.rs");

    for path in rust_files(&source_root()) {
        if path == keymap {
            continue;
        }
        let text = fs::read_to_string(&path).expect("a readable source file");
        assert!(
            !text.contains("KeyCode::"),
            "{} names a KeyCode; only keymap.rs may (ADR-0005)",
            path.display()
        );
    }
}

#[test]
fn the_scan_would_notice_a_key_that_escaped() {
    // A test of the test: the guard above is worth nothing if it cannot see
    // the file it is meant to be reading.
    let keymap = source_root().join("tui").join("keymap.rs");
    let text = fs::read_to_string(&keymap).expect("keymap.rs is readable");

    assert!(text.contains("KeyCode::"), "the keymap names keys itself");
    assert!(
        rust_files(&source_root()).contains(&keymap),
        "the scan reaches nested modules"
    );
}

#[test]
fn dev_hands_its_record_service_to_the_loop() {
    // SPEC-007: `cli::dev` built a `RecordService` and bound it to `_records`
    // because T7's loop had nothing to hand it to. T8's does, so the
    // underscore — the marker of a service nothing could reach — must be gone.
    let cli = fs::read_to_string(source_root().join("cli.rs")).expect("cli.rs is readable");

    assert!(
        !cli.contains("_records"),
        "cli.rs still parks the RecordService in a binding nothing reads"
    );
    assert!(
        cli.contains("tui::run(schema, records)"),
        "cli.rs must hand the service it builds to the loop"
    );
}

#[test]
fn the_crate_still_names_none_of_the_domains_own_dependencies() {
    // SPEC-007 "Purity and boundaries": `Record`, `FieldType` and `Json` reach
    // the UI through `pressa_app::domain`, so this crate takes no dependency of
    // its own to get at them — in any section. `architecture.rs` owns the
    // `pressa-core` and `rusqlite` edges and is deliberately left unchanged;
    // these two are the ones T8 could have been tempted into.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest).expect("Cargo.toml is readable");

    for forbidden in ["serde_json", "chrono"] {
        assert!(
            !manifest.contains(forbidden),
            "pressa-tui/Cargo.toml names {forbidden}; the domain types come \
             through pressa_app::domain instead"
        );
    }
}

/// `text` up to its `#[cfg(test)]`, which is where a file's own unit tests
/// start. A test may name a collection to build a fixture route; the code it
/// tests may not.
fn without_tests(text: &str) -> &str {
    // `split_once` keeps everything before the marker, or all of it when a file
    // has no tests at all.
    text.split_once("#[cfg(test)]")
        .map_or(text, |(code, _)| code)
}

#[test]
fn no_renderer_names_a_collection_slug() {
    // ADR-0004's rule most likely to be broken by a well-meaning agent: one
    // table renders every collection, so no renderer may know what `posts` is.
    // The positive half of this proof is in the snapshots — frames B and D are
    // two different collections through the same `table::render`.
    for path in rust_files(&source_root()) {
        let text = fs::read_to_string(&path).expect("a readable source file");
        let code = without_tests(&text);

        for slug in ["\"posts\"", "\"authors\"", "\"categories\"", "\"entries\""] {
            assert!(
                !code.contains(slug),
                "{} names the collection {slug} (ADR-0004)",
                path.display()
            );
        }
    }
}

#[test]
fn the_slug_scan_reads_the_code_and_not_only_the_tests() {
    // A test of the test: `keymap.rs` names `posts` in a fixture, so a scan
    // that found nothing there would be looking at the wrong half of the file.
    let keymap = fs::read_to_string(source_root().join("tui").join("keymap.rs"))
        .expect("keymap.rs is readable");

    assert!(keymap.contains("\"posts\""), "the fixture is still there");
    assert!(
        !without_tests(&keymap).contains("\"posts\""),
        "and it is below the #[cfg(test)] line, where the scan ignores it"
    );
}
