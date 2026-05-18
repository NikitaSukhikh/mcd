# MCD Project Dependencies

This document lists all dependencies required to build and develop the MCD (Markdown CSV Document) project.

---

## Rust Dependencies

### Core Crate (`mcd-core`)

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonschema = "*"
csv = "1"
zip = "*"
comrak = "*"
thiserror = "1"
indexmap = { version = "*", features = ["serde"] }
sha2 = "*"
rust_decimal = { version = "*", features = ["serde"] }
time = { version = "*", features = ["serde", "parsing", "formatting"] }
camino = "*"
mime_guess = "*"
roxmltree = "*"

[dev-dependencies]
insta = "*"
proptest = "*"
```

### CLI Crate (`mcd-cli`)

```toml
[dependencies]
mcd-core = { path = "../mcd-core" }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

### Render Crate (`mcd-render`)

```toml
[dependencies]
mcd-core = { path = "../mcd-core" }
```

### WASM Crate (`mcd-wasm`)

```toml
[dependencies]
mcd-core = { path = "../mcd-core" }
wasm-bindgen = "*"

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
name = "mcd"
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
  "name": "@mcd/parser",
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

---

## Version Notes

- Pin exact versions before production release
- Wildcard versions (`*`) shown above are for planning only
- Run `cargo update` to get latest compatible versions
- Use `cargo audit` regularly for security updates
