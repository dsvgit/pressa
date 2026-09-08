//! `discover_project` and `--project` — SPEC-001 "Project discovery".

mod support;

use std::path::Path;

use pressa_app::config::{ConfigError, discover_project, project_at};
use support::TempTree;

const MINIMAL: &str = "\
project:
  name: blog-cms
collections:
  posts:
    fields:
      - name: title
        type: text
";

#[test]
fn discover_project_finds_a_config_three_directories_up() {
    let tree = TempTree::new("discover-up");
    tree.write("pressa.yaml", MINIMAL);
    let deep = tree.dir("posts/drafts/2026"); // three levels below the config

    let paths = discover_project(&deep).expect("config found by walking up");

    assert_eq!(paths.root, tree.root());
    assert_eq!(paths.config, tree.root().join("pressa.yaml"));
    assert_eq!(paths.data_dir, tree.root().join(".pressa"));
    assert_eq!(paths.database, tree.root().join(".pressa").join("data.db"));
    assert_eq!(paths.log, tree.root().join(".pressa").join("pressa.log"));
}

#[test]
fn discover_project_finds_a_config_in_the_starting_directory() {
    let tree = TempTree::new("discover-here");
    tree.write("pressa.yaml", MINIMAL);

    let paths = discover_project(tree.root()).expect("config found where we started");

    assert_eq!(paths.root, tree.root());
}

#[test]
fn discover_project_reports_not_found_at_the_filesystem_root_without_looping() {
    // Nothing in this tree, and no pressa.yaml between the temp dir and `/`,
    // so the walk has to stop at the root instead of spinning there.
    let tree = TempTree::new("discover-none");
    let deep = tree.dir("a/b/c");

    let error = discover_project(&deep).expect_err("no config anywhere above");

    match error {
        ConfigError::NotFound { searched_from } => assert_eq!(searched_from, deep),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn project_at_bypasses_discovery() {
    // The config lives at the top of the tree, but `--project` names a nested
    // directory: discovery would find it, `project_at` must not.
    let tree = TempTree::new("project-flag");
    tree.write("pressa.yaml", MINIMAL);
    let nested = tree.dir("nested");

    assert!(discover_project(&nested).is_ok(), "the walk would find it");

    let error = project_at(&nested).expect_err("--project does not walk up");
    assert!(matches!(error, ConfigError::NotFound { .. }), "{error:?}");
}

#[test]
fn project_at_accepts_a_directory_that_holds_the_config() {
    let tree = TempTree::new("project-flag-ok");
    tree.write("pressa.yaml", MINIMAL);

    let paths = project_at(tree.root()).expect("config is right there");

    assert_eq!(paths.root, tree.root());
    assert_eq!(paths.config, tree.root().join("pressa.yaml"));
}

#[test]
fn a_relative_start_is_resolved_against_the_working_directory() {
    // `pressa` is normally launched in `.`, whose `parent()` is None, so the
    // walk stops immediately unless the start is made absolute first. Test
    // binaries run with the crate directory as the working directory.
    let paths = discover_project(Path::new("../examples/blog")).expect("the example project");

    assert!(paths.root.is_absolute(), "{:?}", paths.root);
    assert!(paths.root.ends_with("examples/blog"), "{:?}", paths.root);
    assert!(paths.config.is_file());
}
