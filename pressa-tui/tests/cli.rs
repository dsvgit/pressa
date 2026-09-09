//! The CLI's acceptance criteria — [SPEC-005](../../specs/005-cli.md).
//!
//! Two kinds of test. Anything observable from a function call is a unit test
//! on that function; anything about a *process* — its exit code, and which of
//! stdout and stderr it wrote to — is a spawn of the real binary, which is the
//! whole reason ADR-0010 admits `assert_cmd`.
//!
//! One rule constrains the layout: `logging::init` sets the process-global
//! `tracing` subscriber and refuses a second call, so at most one test in this
//! binary may call an in-process function that initialises logging. That test
//! is `init_scaffolds_a_project_and_starts_its_log`; every other logging
//! assertion is made against a spawned process, which has its own global.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::assert::Assert;
use assert_cmd::prelude::*;
use clap::Parser;
use predicates::prelude::*;

use pressa_app::config::{ProjectPaths, project_at};
use pressa_tui::cli::{self, Cli, CliError, LogLevel};

use support::{TempTree, example_schema_path, workspace_root};

/// The summary line SPEC-000 step 2 asserts, for `examples/blog`.
const BLOG_SUMMARY: &str = "schema ok: 1 collection, 8 fields";

/// Two collections and three fields between them, for the pluralisation case.
const TWO_COLLECTIONS: &str = "\
project:
  name: two

collections:
  posts:
    fields:
      - name: title
        type: text
      - name: slug
        type: text
  authors:
    fields:
      - name: name
        type: text
";

/// Field index 3 is a type M0 does not have, so the loader's message names
/// `collections.posts.fields[3].type` (screen F).
const RELATION_FIELD: &str = "\
project:
  name: broken

collections:
  posts:
    fields:
      - name: title
        type: text
      - name: slug
        type: text
      - name: status
        type: text
      - name: author
        type: relation
";

/// `list_columns` naming a field that does not exist (screen J).
const MISSPELLED_COLUMN: &str = "\
project:
  name: broken

collections:
  posts:
    list_columns: [titel]
    fields:
      - name: title
        type: text
";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The binary under test. `CARGO_BIN_EXE_pressa` is set by cargo for an
/// integration test of the crate that defines the bin, so nothing is resolved
/// by hand (ADR-0010).
fn pressa(cwd: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pressa"));
    // Every command reads the working directory, and no test may change the
    // process's own — so the child gets one instead.
    command.current_dir(cwd);
    command
}

/// SPEC-005's cross-cutting shape for a failing run: exit 1, nothing on
/// stdout, exactly one line on stderr, beginning `pressa: `. Returns that line
/// so the caller can assert on its wording.
fn one_error_line(assert: Assert) -> String {
    let assert = assert.failure().code(1).stdout(predicate::str::is_empty());
    // `get_output` borrows the captured streams the assertion already holds.
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "stderr was: {stderr:?}");
    assert!(
        lines[0].starts_with("pressa: "),
        "stderr was: {stderr:?}, expected it to start with `pressa: `"
    );
    lines[0].to_string()
}

/// A project directory holding a copy of the example schema.
fn project_with(tree: &TempTree, relative: &str, yaml: &str) -> ProjectPaths {
    tree.write(&format!("{relative}/pressa.yaml"), yaml);
    project_at(&tree.root().join(relative)).expect("the fixture project resolves")
}

fn example_yaml() -> String {
    fs::read_to_string(example_schema_path()).expect("examples/blog/pressa.yaml is readable")
}

fn log_of(project: &Path) -> String {
    fs::read_to_string(pressa_tui::logging::log_path(project)).expect("the log file exists")
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[test]
fn help_lists_exactly_three_subcommands() {
    // `command()` is clap's own description of the CLI, so this asserts on what
    // `--help` prints without matching against its layout. `help` is clap's
    // built-in and is not one of ours.
    let command = <Cli as clap::CommandFactory>::command();
    let names: Vec<String> = command
        .get_subcommands()
        .map(|sub| sub.get_name().to_string())
        .filter(|name| name != "help")
        .collect();

    assert_eq!(names, ["dev", "init", "validate"]);

    let tree = TempTree::new("help");
    let assert = pressa(tree.root()).arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    for name in ["dev", "init", "validate"] {
        assert!(stdout.contains(name), "--help was: {stdout}");
    }
}

#[test]
fn no_subcommand_is_the_same_as_dev() {
    let bare = Cli::try_parse_from(["pressa"]).expect("`pressa` parses");
    let dev = Cli::try_parse_from(["pressa", "dev"]).expect("`pressa dev` parses");

    // Absent means `Dev`, so the two resolve to the same command.
    assert_eq!(bare.command.unwrap_or(cli::Command::Dev), cli::Command::Dev);
    assert_eq!(dev.command, Some(cli::Command::Dev));
}

#[test]
fn global_flags_parse_on_either_side_of_the_subcommand() {
    let before = Cli::try_parse_from([
        "pressa",
        "--project",
        "X",
        "--log-level",
        "debug",
        "validate",
    ])
    .expect("flags before the subcommand parse");
    let after = Cli::try_parse_from([
        "pressa",
        "validate",
        "--project",
        "X",
        "--log-level",
        "debug",
    ])
    .expect("flags after the subcommand parse");

    for cli in [&before, &after] {
        assert_eq!(cli.project.as_deref(), Some(Path::new("X")));
        assert_eq!(cli.log_level, LogLevel::Debug);
        assert_eq!(cli.command, Some(cli::Command::Validate));
    }
}

#[test]
fn log_level_defaults_to_info() {
    let cli = Cli::try_parse_from(["pressa", "validate"]).expect("`pressa validate` parses");
    assert_eq!(cli.log_level, LogLevel::Info);
}

#[test]
fn an_unknown_log_level_is_a_usage_error() {
    let error = Cli::try_parse_from(["pressa", "--log-level", "nonsense", "validate"])
        .expect_err("`nonsense` is not a level");
    assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);

    // A usage error exits 2, and the schema is never reached: the run happens
    // inside a valid project and still prints clap's message, not ours.
    let tree = TempTree::new("bad-level");
    tree.write("pressa.yaml", &example_yaml());
    let assert = pressa(tree.root())
        .args(["--log-level", "nonsense", "validate"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("nonsense"));
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(!stderr.contains("schema ok"), "stderr was: {stderr}");
}

#[test]
fn an_unknown_subcommand_is_a_usage_error() {
    let tree = TempTree::new("bad-subcommand");
    pressa(tree.root())
        .arg("publish")
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

#[test]
fn init_scaffolds_a_project_and_starts_its_log() {
    // The one in-process test that initialises logging — see the module note.
    let tree = TempTree::new("init");

    cli::init(tree.root(), LogLevel::Info).expect("init scaffolds an empty directory");

    // Byte-for-byte, because `init` embeds the example file itself.
    assert_eq!(
        fs::read_to_string(tree.root().join("pressa.yaml")).expect("pressa.yaml was written"),
        example_yaml()
    );
    assert!(tree.root().join(".pressa").is_dir(), ".pressa/ was created");
    assert!(
        !tree.root().join(".pressa").join("data.db").exists(),
        "the database is created lazily, on the first `dev`"
    );
    assert!(
        !log_of(tree.root()).is_empty(),
        "init's success path leaves a log"
    );
}

#[test]
fn a_scaffolded_project_validates() {
    let tree = TempTree::new("init-then-validate");

    pressa(tree.root()).arg("init").assert().success();
    pressa(tree.root())
        .arg("validate")
        .assert()
        .success()
        .stdout(format!("{BLOG_SUMMARY}\n"));
}

#[test]
fn init_with_project_scaffolds_into_that_directory() {
    let tree = TempTree::new("init-project");

    pressa(tree.root())
        .args(["init", "--project", "nested/blog"])
        .assert()
        .success();

    assert!(tree.root().join("nested/blog/pressa.yaml").is_file());
    assert!(tree.root().join("nested/blog/.pressa").is_dir());
    // The working directory is left alone: `--project` is the target, not a
    // second place to scaffold.
    assert!(!tree.root().join("pressa.yaml").exists());
    assert!(!tree.root().join(".pressa").exists());
}

#[test]
fn init_refuses_to_overwrite_an_existing_project() {
    let tree = TempTree::new("init-refuses");
    let existing = "# mine, hand-written\n";
    tree.write("pressa.yaml", existing);

    let line = one_error_line(pressa(tree.root()).arg("init").assert());
    assert!(
        line.contains("pressa.yaml already exists in"),
        "stderr was: {line}"
    );

    // Nothing was written: not the file, not `.pressa/`, not the log.
    assert_eq!(
        fs::read_to_string(tree.root().join("pressa.yaml")).expect("the file is still there"),
        existing
    );
    assert!(!tree.root().join(".pressa").exists());
}

#[test]
fn init_reuses_an_existing_dot_pressa_directory() {
    let tree = TempTree::new("init-reuse");
    tree.write(".pressa/keep.txt", "keep me");

    pressa(tree.root()).arg("init").assert().success();

    assert!(tree.root().join("pressa.yaml").is_file());
    assert_eq!(
        fs::read_to_string(tree.root().join(".pressa/keep.txt")).expect("the file survived"),
        "keep me"
    );
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

#[test]
fn validate_summarises_the_example_schema() {
    let paths = project_at(&workspace_root().join("examples").join("blog"))
        .expect("examples/blog is a project");
    assert_eq!(
        cli::validate(&paths).expect("the example loads"),
        BLOG_SUMMARY
    );
}

#[test]
fn validate_pluralises_both_nouns_independently() {
    let tree = TempTree::new("validate-plural");
    let paths = project_with(&tree, "two", TWO_COLLECTIONS);
    assert_eq!(
        cli::validate(&paths).expect("the fixture loads"),
        "schema ok: 2 collections, 3 fields"
    );
}

#[test]
fn validate_prints_the_summary_and_nothing_else() {
    let tree = TempTree::new("validate-stdout");
    let blog = workspace_root().join("examples").join("blog");

    pressa(tree.root())
        .args(["validate", "--project"])
        .arg(&blog)
        .assert()
        .success()
        .stdout(format!("{BLOG_SUMMARY}\n"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn validate_reports_the_yaml_path_of_a_bad_field_type() {
    let tree = TempTree::new("validate-relation");
    tree.write("pressa.yaml", RELATION_FIELD);

    let line = one_error_line(pressa(tree.root()).arg("validate").assert());
    assert!(
        line.contains("collections.posts.fields[3].type"),
        "stderr was: {line}"
    );
}

#[test]
fn validate_without_a_project_says_so_and_creates_nothing() {
    let tree = TempTree::new("validate-no-project");
    let scratch = tree.dir("scratch");

    let line = one_error_line(pressa(&scratch).arg("validate").assert());
    assert!(line.contains("no pressa.yaml found"), "stderr was: {line}");
    // A directory that is not a project must not be given a `.pressa/`.
    assert!(!scratch.join(".pressa").exists());
}

#[test]
fn validate_with_project_does_not_walk_up() {
    let tree = TempTree::new("validate-project-bypass");
    tree.write("pressa.yaml", &example_yaml()); // a valid project above
    let empty = tree.dir("empty");

    let line = one_error_line(
        pressa(tree.root())
            .args(["validate", "--project"])
            .arg(&empty)
            .assert(),
    );
    assert!(line.contains("no pressa.yaml found"), "stderr was: {line}");
    assert!(
        line.contains(&empty.display().to_string()),
        "the message names the directory asked for: {line}"
    );
}

#[test]
fn validate_leaves_a_log_in_the_project_it_validated() {
    let tree = TempTree::new("validate-log");
    tree.write("blog/pressa.yaml", &example_yaml());
    let project = tree.root().join("blog");

    pressa(tree.root())
        .args(["validate", "--project"])
        .arg(&project)
        .assert()
        .success();

    assert!(!log_of(&project).is_empty(), "validate's success path logs");
}

// ---------------------------------------------------------------------------
// resolve_project
// ---------------------------------------------------------------------------

#[test]
fn resolve_project_discovers_upward_from_a_nested_directory() {
    let tree = TempTree::new("resolve-discovery");
    tree.write("pressa.yaml", &example_yaml());
    let nested = tree.dir("a/b/c");

    let paths = cli::resolve_project(None, &nested).expect("discovery finds the project above");
    assert_eq!(paths.config, tree.root().join("pressa.yaml"));
}

#[test]
fn resolve_project_with_project_bypasses_discovery() {
    let tree = TempTree::new("resolve-bypass");
    tree.write("pressa.yaml", &example_yaml()); // the project discovery would find
    tree.write("other/pressa.yaml", &example_yaml());
    let other = tree.root().join("other");

    let paths = cli::resolve_project(Some(&other), tree.root()).expect("--project wins");
    assert_eq!(paths.root, other);
}

#[test]
fn resolve_project_reports_a_missing_project() {
    let tree = TempTree::new("resolve-missing");
    let error = cli::resolve_project(None, tree.root()).expect_err("there is no project here");

    // The wording is `ConfigError`'s, carried through untouched.
    assert!(matches!(error, CliError::Config(_)), "error was: {error:?}");
    assert!(error.to_string().contains("no pressa.yaml found"));
}

// ---------------------------------------------------------------------------
// dev
// ---------------------------------------------------------------------------

#[test]
fn dev_does_every_startup_step_and_then_stops_at_the_screen() {
    let tree = TempTree::new("dev");
    tree.write("pressa.yaml", &example_yaml());

    let line = one_error_line(pressa(tree.root()).arg("dev").assert());
    assert_eq!(line, "pressa: the TUI arrives in T7");

    // The database was created and migrated on the way.
    let database = tree.root().join(".pressa").join("data.db");
    assert!(database.is_file(), "dev creates the database");
    // Opened through the storage adapter, which runs the migration check: a
    // file this accepts is one the app can work with. The tables are then
    // looked for in the file itself, because listing records would mean naming
    // `pressa-core` types, and `pressa-tui` has no such dependency (ADR-0006).
    pressa_storage::SqliteRepository::open(&database).expect("the database opens and migrates");
    let bytes = fs::read(&database).expect("the database is readable");
    for table in ["records", "collections", "meta", "schema_version"] {
        assert!(
            bytes
                .windows(table.len())
                .any(|window| window == table.as_bytes()),
            "the migrated schema mentions {table}"
        );
    }

    // And the log says the services were built before the stub gave up.
    let log = log_of(tree.root());
    assert!(log.contains("services ready"), "log was: {log}");
}

#[test]
fn no_subcommand_behaves_like_dev() {
    let tree = TempTree::new("dev-default");
    tree.write("pressa.yaml", &example_yaml());

    let line = one_error_line(pressa(tree.root()).assert());
    assert_eq!(line, "pressa: the TUI arrives in T7");
    assert!(tree.root().join(".pressa").join("data.db").is_file());
}

#[test]
fn dev_on_a_bad_schema_leaves_no_database() {
    let tree = TempTree::new("dev-bad-schema");
    tree.write("pressa.yaml", MISSPELLED_COLUMN);

    let line = one_error_line(pressa(tree.root()).arg("dev").assert());
    assert!(
        line.contains("collections.posts.list_columns[0]") && line.contains("titel"),
        "stderr was: {line}"
    );
    // The schema is loaded before the database is opened.
    assert!(!tree.root().join(".pressa").join("data.db").exists());
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

#[test]
fn log_level_error_drops_every_event_the_default_level_keeps() {
    let tree = TempTree::new("log-level");
    tree.write("pressa.yaml", &example_yaml());

    // `dev`'s events are INFO and WARN, so `--log-level error` writes nothing.
    pressa(tree.root())
        .args(["--log-level", "error", "dev"])
        .assert()
        .failure();
    assert!(
        log_of(tree.root()).is_empty(),
        "an event above the level never reaches the file"
    );

    // The same project at the default level, appending to the same file.
    pressa(tree.root()).arg("dev").assert().failure();
    assert!(!log_of(tree.root()).is_empty(), "the default level logs");
}
