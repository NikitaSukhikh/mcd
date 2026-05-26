# Release Checklist

Use this order for the first public release.

## Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix bindings/typescript
npm run build --prefix bindings/typescript
cd bindings/python
python -m pytest
python -m maturin build --sdist --out dist --compatibility pypi
cd ../..
```

PHP verification requires PHP and Composer:

```bash
cargo install --path crates/mcd-cli
cd bindings/php
composer validate --strict
composer test
```

## Publish Rust

Publish `mcd-core` first. The other Rust crates cannot be packaged for crates.io
until `mcd-core` exists in the registry.

```bash
cargo publish --manifest-path crates/mcd-core/Cargo.toml
cargo publish --manifest-path crates/mcd-query/Cargo.toml
cargo publish --manifest-path crates/mcd-render/Cargo.toml
cargo publish --manifest-path crates/mcd-wasm/Cargo.toml
cargo publish --manifest-path crates/mcd-cli/Cargo.toml
cargo publish --manifest-path crates/mcd-mcp/Cargo.toml
```

## Publish TypeScript

Create an npm automation token and store it as the GitHub Actions repository
secret `NPM_TOKEN`. The package is scoped as `@mcd-nix/parser`, so the npm
account or organization must own the `@mcd-nix` scope.

```bash
cd bindings/typescript
npm ci
npm test
npm run build
npm pack --dry-run
npm publish --access public
```

## Publish Python

The PyPI distribution is `mcdee`; the import package remains `mcd`.
Python wheels are built with PyO3's stable ABI for CPython 3.9 and newer, so a
single wheel per operating system/architecture can serve Python 3.9 through
newer CPython releases.

PyPI Trusted Publisher settings:

| Field | Value |
| --- | --- |
| PyPI project name | `mcdee` |
| Owner | `NikitaSukhikh` |
| Repository name | `mcd` |
| Workflow name | `release.yml` |
| Environment name | `pypi` |

```bash
cd bindings/python
python -m pip install --upgrade maturin
python -m maturin build --sdist --out dist --compatibility pypi
python -m maturin upload dist/*
```

The release workflow builds and publishes:

| Platform | Wheel coverage |
| --- | --- |
| Linux x64 | `cp39-abi3-manylinux2014_x86_64` |
| Linux arm64 | `cp39-abi3-manylinux2014_aarch64` |
| macOS universal2 | `cp39-abi3-macosx_*_universal2` |
| Windows x64 | `cp39-abi3-win_amd64` |

After publishing, verify that a supported machine can install without source
build tools:

```bash
python -m pip install --only-binary=:all: mcdee
```

## Publish PHP

Packagist publishes from Git tags. After pushing the release tag, submit or
update the `mcd-nix/parser` package in Packagist with this repository URL.

Packagist package settings:

| Field | Value |
| --- | --- |
| Repository URL | `https://github.com/NikitaSukhikh/mcd-php` |
| Package name | `mcd-nix/parser` |
| Composer file | `composer.json` |

The `mcd-php` repository is a split package containing the contents of
`bindings/php` at its root.

## Tag

After registry verification, tag the release:

```bash
git tag v0.1.0-alpha.2
git push origin v0.1.0-alpha.2
```

The tag workflow uploads Windows, macOS, and Linux CLI and MCP server binaries
to the GitHub Release.

CLI archives are built for:

| Platform | Archive |
| --- | --- |
| Linux x64 | `mcd-cli-linux-x64.tar.gz` |
| Linux arm64 | `mcd-cli-linux-arm64.tar.gz` |
| macOS x64 | `mcd-cli-macos-x64.tar.gz` |
| macOS arm64 | `mcd-cli-macos-arm64.tar.gz` |
| Windows x64 | `mcd-cli-windows-x64.zip` |

MCP server archives are built for:

| Platform | Archive |
| --- | --- |
| Linux x64 | `mcd-mcp-linux-x64.tar.gz` |
| Linux arm64 | `mcd-mcp-linux-arm64.tar.gz` |
| macOS x64 | `mcd-mcp-macos-x64.tar.gz` |
| macOS arm64 | `mcd-mcp-macos-arm64.tar.gz` |
| Windows x64 | `mcd-mcp-windows-x64.zip` |

Each archive is uploaded with a matching `.sha256` checksum file.
