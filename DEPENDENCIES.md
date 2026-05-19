# MCD Project Dependencies

This document lists all dependencies required to build and develop the MCD (Markdown CSV Document) project.

---

## Rust Dependencies

### Core Crate (`mcd-core`)

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonschema = "0.46.5"
csv = "1"
zip = "8.6.0"
comrak = "0.52.0"
thiserror = "1"
indexmap = { version = "2.14.0", features = ["serde"] }
sha2 = "0.11.0"
rust_decimal = { version = "1.42.0", features = ["serde"] }
time = { version = "0.3.47", features = ["serde", "parsing", "formatting"] }
camino = "1.2.2"
mime_guess = "2.0.5"
roxmltree = "0.21.1"

[dev-dependencies]
insta = "1.47.2"
proptest = "1.11.0"
```

### CLI Crate (`mcd-cli`)

```toml
[dependencies]
mcd-core = { version = "0.1.0-alpha.0", path = "../mcd-core" }
mcd-render = { version = "0.1.0-alpha.0", path = "../mcd-render" }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

### Render Crate (`mcd-render`)

```toml
[dependencies]
mcd-core = { version = "0.1.0-alpha.0", path = "../mcd-core" }
```

### WASM Crate (`mcd-wasm`)

```toml
[dependencies]
mcd-core = { version = "0.1.0-alpha.0", path = "../mcd-core" }

[lib]
crate-type = ["cdylib"]
```

---

## Python Dependencies

### Build Tools

| Tool | Purpose |
|------|---------|
| PyO3 0.23 | Rust-Python bindings |
| maturin >=1.7,<2 | Build system for PyO3 projects |

### pyproject.toml

```toml
[build-system]
requires = ["maturin>=1.7,<2"]
build-backend = "maturin"

[project]
name = "mcdee"
requires-python = ">=3.9"
classifiers = [
    "Programming Language :: Rust",
    "Programming Language :: Python :: Implementation :: CPython",
]

[project.optional-dependencies]
pandas = ["pandas>=1.5"]
test = ["pytest>=8", "maturin>=1.7", "pandas>=1.5"]

[tool.maturin]
module-name = "mcd._native"
python-source = "."
features = ["pyo3/extension-module"]
```

### Development Dependencies

```txt
pytest
ruff
mypy
pandas (optional)
```

---

## TypeScript/JavaScript Dependencies

### package.json

```json
{
  "name": "@mcd-nix/parser",
  "type": "module",
  "devDependencies": {
    "typescript": "^5.0.0",
    "vitest": "^1.0.0",
    "playwright": "^1.40.0"
  }
}
```

### Build Tools

| Tool | Purpose |
|------|---------|
| wasm-pack | Build Rust to WASM |
| wasm-bindgen | Rust-JS bindings |

---

## PHP Dependencies

### composer.json

```json
{
  "name": "mcd-nix/parser",
  "require": {
    "php": ">=8.1"
  },
  "autoload": {
    "psr-4": {
      "Mcd\\": "src/"
    }
  }
}
```

The PHP wrapper has no runtime package dependencies, but it requires the `mcd` CLI binary to be available on `PATH` or passed to `Mcd\Client`.

---

## CI/CD Tools

| Tool | Purpose |
|------|---------|
| cargo fmt | Rust formatting |
| cargo clippy | Rust linting |
| cargo test | Rust testing |
| cargo audit | Security audit |
| cargo fuzz | Fuzzing (later) |
| cargo-release | Release automation (optional) |

---

## Rendering Dependencies (Optional)

| Tool | Purpose |
|------|---------|
| WeasyPrint | HTML/CSS to PDF (open-source) |
| PrinceXML | HTML/CSS to PDF (commercial) |
| Headless Chromium | Browser-based PDF export |

---

## Summary Table

| Ecosystem | Package Manager | Key Dependencies |
|-----------|----------------|------------------|
| Rust | Cargo | serde, serde_json, jsonschema, csv, zip, comrak, clap, thiserror, anyhow, time, rust_decimal, indexmap, sha2, camino, wasm-bindgen |
| Python | pip/maturin | PyO3, maturin, pytest, ruff, mypy, pandas (optional) |
| TypeScript | npm | typescript, vitest, playwright, wasm-pack |
| PHP | Composer | PHP >=8.1, installed `mcd` CLI |

---

## Installation Commands

### Rust Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack
```

### Python Setup

```bash
# Install maturin
pip install maturin

# Build Python bindings
cd bindings/python
maturin develop
```

### TypeScript Setup

```bash
# Install dependencies
cd bindings/typescript
npm install

# Build WASM package
cd ../../crates/mcd-wasm
wasm-pack build --target web
```

### PHP Setup

```bash
# Install wrapper dependencies
cd bindings/php
composer install

# Run PHP wrapper tests
composer test
```

---

## Version Notes

- Publishable Rust manifests use explicit version requirements.
- Run `cargo update` to get latest compatible versions
- Use `cargo audit` regularly for security updates
