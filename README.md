# alfred-caniuse-rs

> Rust feature support from [caniuse.rs](https://caniuse.rs) as an Alfred workflow.

# Installation

Go to [releases](https://github.com/robjtede/alfred-caniuse-rs/releases) and download the `*.alfredworkflow` file from the most recent release. Open workflow file for Alfred to prompt the install.

# Features

- [x] Fuzzy feature matching
- [x] Fast results
- [x] Feature db caching
- [x] Recent quick version lookup
- [x] Self update notifications

Alfred keyword is `caniuse`.

## Rust Feature Lookup

- Type a query after caniuse.
- Option+Enter to open item's documentation.

## Recent Version Summary

- No query to see recent version list.
- Shift to preview release blog post.
- Option+Enter to open release blog post in browser.

# Releases

[Release-plz](https://release-plz.dev/docs/config) opens a draft release PR with a version update and a generated `CHANGELOG.md`. Use conventional commit messages to describe changes for the changelog.

Merge the release PR to create a draft GitHub release. The workflow then builds a universal macOS binary for Apple Silicon and Intel Macs and uploads these assets:

- `caniuse.rs-<version>.alfredworkflow`: the installable Alfred workflow.
- `alfred-caniuse-rs-universal-apple-darwin.tar.gz`: the standalone binary.
- `package.zip`: a copy of the Alfred workflow for the update checker.
- `SHA256SUMS`: SHA-256 checksums for the archives.

The workflow generates build provenance and publishes the draft release after all assets are uploaded. The [x52 release tools](https://github.com/x52dev/nix/tree/main/release-tools) add release status comments to the release PR. Releases use Git tags to track versions and require only `GITHUB_TOKEN`. In the repository's Actions settings, enable **Allow GitHub Actions to create and approve pull requests**.

To build the same assets locally on macOS, install both Rust targets and run the release recipe:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
just build-release
```

The files are written to `target/release-assets/`. Packaging reads the version from `Cargo.toml` and sets it in a copy of `workflow/info.plist`.

# Screenshots

| Features                                                                                                  | Version                                                                                                   |
| --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| ![](https://user-images.githubusercontent.com/3316789/111861731-72b44700-8948-11eb-8cf4-b11987318333.png) | ![](https://user-images.githubusercontent.com/3316789/111861730-7051ed00-8948-11eb-9699-d15e5bb6f5e0.png) |
