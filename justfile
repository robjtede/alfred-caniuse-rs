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
    cargo clippy --workspace --all-features --fix {{ args }}

# Test workspace.
test:
    cargo test --workspace --no-default-features
    cargo test --workspace --all-features

# Build release assets for Apple Silicon and Intel Macs.
build-release:
    cargo build --locked --release --target aarch64-apple-darwin --target x86_64-apple-darwin

    mkdir -p target/universal-apple-darwin/release
    lipo -create target/aarch64-apple-darwin/release/alfred-caniuse-rs target/x86_64-apple-darwin/release/alfred-caniuse-rs -output target/universal-apple-darwin/release/alfred-caniuse-rs
    codesign --force --sign - target/universal-apple-darwin/release/alfred-caniuse-rs

    just package-release target/universal-apple-darwin/release/alfred-caniuse-rs

# Package a macOS binary as an Alfred workflow and a standalone archive.
package-release binary:
    #!/usr/bin/env bash
    set -euo pipefail

    version=$(cargo metadata --locked --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "alfred-caniuse-rs") | .version')
    assets_dir="$PWD/target/release-assets"

    mkdir -p "$assets_dir"

    workflow_dir=$(mktemp -d "$PWD/target/alfredworkflow.XXXXXX")
    trap 'rm -rf "$workflow_dir"' EXIT

    install -m 755 "{{ binary }}" "$workflow_dir/caniuse"
    cp workflow/info.plist "$workflow_dir/info.plist"
    cp assets/alfred-caniuse-rs.png "$workflow_dir/icon.png"
    cp assets/alfred-caniuse-rs.png "$workflow_dir/54823BEA-0489-434F-82C9-0A979FEE1D41.png"

    plutil -replace version -string "$version" "$workflow_dir/info.plist"
    plutil -lint "$workflow_dir/info.plist"

    workflow_archive="caniuse.rs-${version}.alfredworkflow"
    binary_archive="alfred-caniuse-rs-universal-apple-darwin.tar.gz"

    rm -f "$assets_dir/$workflow_archive"

    (cd "$workflow_dir" && zip -q "$assets_dir/$workflow_archive" ./*)
    cp "$assets_dir/$workflow_archive" "$assets_dir/package.zip"
    tar -czf "$assets_dir/$binary_archive" -C "$(dirname "{{ binary }}")" "$(basename "{{ binary }}")"

    cd "$assets_dir"
    shasum -a 256 "$workflow_archive" "$binary_archive" package.zip > SHA256SUMS

# Document workspace.
doc:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Document workspace and watch for changes.
doc-watch:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features --open
    cargo watch -- RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Check project formatting.
check: && clippy
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
