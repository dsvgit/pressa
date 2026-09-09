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
