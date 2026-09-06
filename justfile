# Pressa development commands. Run `just ci` before finishing any task.

default:
    @just --list

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

check:
    cargo check --workspace --all-targets

# Everything CI runs. Must be green before a task is done.
ci:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Review snapshot diffs deliberately. Never `insta accept` blindly.
snap:
    cargo insta review

# Open the example project.
run:
    cargo run -p pressa-tui -- dev --project examples/blog

validate:
    cargo run -p pressa-tui -- validate --project examples/blog
