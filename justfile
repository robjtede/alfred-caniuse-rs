_list:
    @just --list

# Lint workspace.
clippy:
    cargo clippy --workspace --no-default-features
    cargo clippy --workspace --all-features
    cargo hack --feature-powerset --depth=3 clippy --workspace

# Lint workspace and watch for changes.
clippy-watch:
    cargo watch -- cargo clippy --workspace --all-features

# Apply possible linting fixes in the workspace.
clippy-fix *args:
    cargo clippy --workspace --all-features --fix {{args}}

# Test workspace.
test:
    cargo test --workspace --no-default-features
    cargo test --workspace --all-features

# Document workspace.
doc:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Document workspace and watch for changes.
doc-watch:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features --open
    cargo watch -- RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Check project formatting.
check:
    just --unstable --fmt --check
    npx -y prettier --check '**/*.md'
    taplo lint
    cargo +nightly fmt -- --check

# Format project.
fmt:
    just --unstable --fmt
    npx -y prettier --write '**/*.md'
    taplo format
    cargo +nightly fmt
