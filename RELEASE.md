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
python -m maturin build --sdist --out dist
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
cargo publish --manifest-path crates/mcd-render/Cargo.toml
cargo publish --manifest-path crates/mcd-wasm/Cargo.toml
cargo publish --manifest-path crates/mcd-cli/Cargo.toml
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
python -m maturin publish
```

## Publish PHP

Packagist publishes from Git tags. After pushing the release tag, submit or
update the `mcd/parser` package in Packagist with this repository URL.

## Tag

After registry verification, tag the release:

```bash
git tag v0.1.0-alpha.0
git push origin v0.1.0-alpha.0
```

The tag workflow uploads Windows, macOS, and Linux CLI binaries to the GitHub
Release.
