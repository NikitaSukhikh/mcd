# MCD Implementation Tech Stack

**MCD** means **Markdown CSV Document**.

This document defines the recommended implementation stack for the `.mcd` format: parser, validator, CLI, Python library, renderer, web bindings, tests, packaging, and release tooling.

The core technical decision is:

```text
One canonical parser implementation
→ exposed through multiple interfaces
→ no duplicated parsing logic
```

Recommended stack:

```text
Core parser and validator: Rust
CLI: Rust
Python API: PyO3 + maturin
Web / Node API: Rust → WebAssembly → TypeScript wrapper
Renderer: separate layer, initially HTML/CSS-first
Schemas: JSON-based
Tables: CSV + schema, with optional future Arrow/Parquet export
Package: ZIP-like `.mcd` container
```

---

## 1. Implementation goals

The implementation must make `.mcd` documents feel as easy to consume as PDFs, but without PDF extraction ambiguity.

A parser must be able to:

```text
- open `.mcd` packages
- read `mimetype`
- read `manifest.json`
- read `content/main.md`
- parse Markdown into document blocks
- detect table anchors
- resolve table anchors to CSV tables
- load table schemas
- type-check table data
- load table views
- load layout metadata
- load page maps
- generate expanded Markdown on demand
- generate canonical JSON document streams
- validate conformance profiles
```

The parser should not reverse-engineer visual content. It should read declared structure.

```text
PDF parser:
  visual glyphs + coordinates → guessed text/tables/order

MCD parser:
  Markdown + table anchors + CSV + schema → exact text/tables/order
```

---

## 2. High-level architecture

```text
.mcd package
  │
  ▼
mcd-core  ───────────────┐
  │                       │
  ├─ package reader       │
  ├─ manifest parser      │
  ├─ Markdown parser      │
  ├─ table resolver       │
  ├─ CSV/schema validator │
  ├─ layout/page-map API  │
  ├─ export API           │
  └─ conformance engine   │
                          │
                          ├─ mcd-cli
                          ├─ mcd-python
                          ├─ mcd-wasm / mcd-js
                          └─ mcd-render
```

The Rust core is the source of truth. Other interfaces are wrappers around it.

---

## 3. Primary language choice

## 3.1 Core language: Rust

Use **Rust** for the core parser, validator, package reader, and CLI.

Reasons:

```text
- native performance
- memory safety
- good package/archive handling
- strong type system
- reliable parsing
- good CSV/JSON ecosystem
- strong CLI ecosystem
- good Python binding support
- good WebAssembly path
- cross-platform binaries
```

The core parser will process untrusted files, so memory safety and strict path validation matter.

## 3.2 User-facing language: Python first

Use **Python** as the first public API because AI tooling, notebooks, data science, report automation, and agent pipelines already use Python heavily.

Target experience:

```python
import mcd

doc = mcd.open("annual-report.mcd")
doc.validate()

print(doc.markdown(expand_tables=True))

revenue = doc.table("revenue")
print(revenue.schema)
print(revenue.rows())
```

## 3.3 Web language: TypeScript second

Use **TypeScript** for web viewers, browser plugins, document portals, and Node-based agent tools.

The TypeScript package should wrap the same Rust parser compiled to WebAssembly.

Target experience:

```ts
import { openMcd } from "@mcd/parser";

const doc = await openMcd(file);
const blocks = doc.blocks();
const table = doc.table("revenue");

console.log(doc.markdown({ expandTables: true }));
console.log(table.rows());
```

---

## 4. Repository layout

Recommended monorepo layout:

```text
mcd/
  README.md
  ABOUT.md
  TECH_STACK.md
  SPEC.md
  ROADMAP.md
  LICENSE

  crates/
    mcd-core/
      Cargo.toml
      src/
        lib.rs
        package.rs
        manifest.rs
        markdown.rs
        directives.rs
        tables.rs
        schema.rs
        table_view.rs
        layout.rs
        page_map.rs
        document.rs
        validate.rs
        export.rs
        errors.rs

    mcd-cli/
      Cargo.toml
      src/
        main.rs
        commands/
          validate.rs
          inspect.rs
          extract.rs
          render.rs
          init.rs

    mcd-render/
      Cargo.toml
      src/
        lib.rs
        html.rs
        pdf.rs
        page_map.rs

    mcd-wasm/
      Cargo.toml
      src/
        lib.rs

  bindings/
    python/
      pyproject.toml
      Cargo.toml
      src/
        lib.rs
      mcd/
        __init__.py
        py.typed

    typescript/
      package.json
      tsconfig.json
      src/
        index.ts
        types.ts

  schemas/
    manifest.schema.json
    table.schema.json
    table-view.schema.json
    styles.schema.json
    page-map.schema.json

  examples/
    minimal/
      unpacked/
        mimetype
        manifest.json
        content/main.md
      minimal.mcd

    annual-report/
      unpacked/
        mimetype
        manifest.json
        content/main.md
        tables/revenue.csv
        tables/revenue.schema.json
        tables/revenue.view.json
        layout/styles.json
      annual-report.mcd

  tests/
    fixtures/
      valid/
      invalid/
      security/
      conformance/

  docs/
    parser-api.md
    cli.md
    python.md
    table-directives.md
    validation.md
    rendering.md
```

---

## 5. Rust workspace

The Rust workspace should contain at least these crates:

```text
mcd-core
  Canonical parser, validator, and export engine.

mcd-cli
  Command-line interface built on mcd-core.

mcd-render
  Rendering layer for HTML, PDF, and page maps.

mcd-wasm
  WebAssembly wrapper around mcd-core.
```

The core parser should have no dependency on Python, Node, browser APIs, or AI-specific logic.

---

## 6. Core Rust dependencies

The exact dependency set can evolve, but this is the recommended starting point.

| Area | Recommended crate | Purpose |
|---|---|---|
| Serialization | `serde` | Strongly typed JSON/data serialization |
| JSON | `serde_json` | `manifest.json`, schemas, views, layout |
| JSON Schema validation | `jsonschema` | Validate JSON documents against JSON Schema |
| CSV | `csv` | Read and validate table data |
| ZIP packages | `zip` | Read and write `.mcd` packages |
| Markdown | `comrak` | Parse CommonMark/GFM-style Markdown |
| CLI | `clap` | Command-line argument parsing |
| Error handling | `thiserror` | Structured library errors |
| CLI error handling | `anyhow` | Application-level CLI errors |
| Dates/times | `chrono` or `time` | Date/datetime parsing and validation |
| Decimal numbers | `rust_decimal` | Exact decimal table values |
| Ordered maps | `indexmap` | Stable output ordering |
| Hashing | `sha2` | Checksums and integrity metadata |
| Path normalization | `camino` | UTF-8 path handling |
| Testing | `insta` | Snapshot tests for parser/export output |
| Property tests | `proptest` | Fuzz-like validation of parser invariants |

Recommended default core dependency policy:

```text
- Keep mcd-core small.
- Avoid large rendering dependencies in mcd-core.
- Avoid network clients in mcd-core.
- Avoid runtime scripting engines.
- Avoid dependencies that execute untrusted code.
```

---

## 7. Package reader

Crate module:

```text
mcd_core::package
```

Responsibilities:

```text
- open `.mcd` files
- verify root `mimetype`
- read `manifest.json`
- expose file contents through safe paths
- reject unsafe archive entries
- enforce size limits
- prevent path traversal
- detect duplicate normalized paths
```

Security requirements:

```text
Reject absolute paths.
Reject paths containing `..` traversal.
Reject duplicate paths after normalization.
Reject symlinks if ZIP implementation exposes them.
Limit maximum compressed size.
Limit maximum decompressed size.
Limit maximum number of files.
Limit maximum single-file size.
Do not automatically extract to disk unless explicitly requested.
```

Recommended API:

```rust
let package = McdPackage::open("report.mcd")?;
let manifest_bytes = package.read("manifest.json")?;
let markdown = package.read_to_string("content/main.md")?;
```

---

## 8. Manifest parser

Crate module:

```text
mcd_core::manifest
```

Manifest format:

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md",
  "tables": [
    {
      "id": "revenue",
      "data": "tables/revenue.csv",
      "schema": "tables/revenue.schema.json",
      "views": {
        "default": "tables/revenue.view.json"
      }
    }
  ],
  "layout": {
    "styles": "layout/styles.json",
    "pageMap": "layout/page-map.json"
  }
}
```

Responsibilities:

```text
- parse manifest JSON
- validate required fields
- validate path references
- validate table IDs are unique
- validate table paths exist
- validate declared profile
- validate version compatibility
```

Recommended Rust model:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: String,
    pub profile: McdProfile,
    pub entrypoint: String,
    pub tables: Vec<TableManifestEntry>,
    pub layout: Option<LayoutManifestEntry>,
}
```

---

## 9. Markdown parser

Crate module:

```text
mcd_core::markdown
mcd_core::directives
```

Base dialect:

```text
CommonMark-compatible Markdown
+ MCD table directive extension
+ optional math convention
```

Recommended table directive syntax:

```markdown
:::table
ref: revenue-table
table: revenue
view: default
caption: Revenue by quarter
numbering: auto
:::
```

Parser responsibilities:

```text
- parse Markdown into a block stream
- preserve source positions where possible
- identify headings, paragraphs, lists, quotes, code, math text
- detect MCD table directives
- parse directive fields
- reject invalid table directive syntax in strict mode
- produce canonical block IDs
```

Recommended canonical block model:

```rust
pub enum DocumentBlock {
    Heading(HeadingBlock),
    Paragraph(ParagraphBlock),
    List(ListBlock),
    CodeBlock(CodeBlock),
    Quote(QuoteBlock),
    MathBlock(MathBlock),
    TableRef(TableRefBlock),
}
```

Table ref model:

```rust
pub struct TableRefBlock {
    pub source_id: SourceId,
    pub ref_id: Option<String>,
    pub table_id: String,
    pub view_id: Option<String>,
    pub caption: Option<String>,
    pub numbering: Option<NumberingRule>,
    pub source_span: SourceSpan,
}
```

---

## 10. Table layer

Crate module:

```text
mcd_core::tables
mcd_core::schema
mcd_core::table_view
```

Responsibilities:

```text
- read CSV table data
- parse table schema JSON
- parse table view JSON
- validate CSV headers against schema
- coerce raw CSV strings into typed values
- validate nullability
- validate primitive types
- validate enum values
- validate min/max constraints
- validate view references
- expose exact rows and formatted display rows
```

Supported v0.1 primitive types:

```text
string
integer
decimal
boolean
date
datetime
time
enum
```

Recommended internal typed value model:

```rust
pub enum Value {
    Null,
    String(String),
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(Date),
    DateTime(DateTime),
    Time(Time),
    Enum(String),
}
```

Recommended table API:

```rust
let table = doc.table("revenue")?;
let schema = table.schema();
let rows = table.rows();
let display = table.display_rows("default")?;
```

---

## 11. Validation engine

Crate module:

```text
mcd_core::validate
```

The validator should produce structured diagnostics, not just pass/fail.

Diagnostic levels:

```text
error
warning
info
```

Example validation result:

```json
{
  "valid": false,
  "profile": "MCD-Core",
  "diagnostics": [
    {
      "level": "error",
      "code": "table.anchor.unresolved",
      "message": "Table anchor references unknown table `revenue-2026`.",
      "source": "content/main.md:14-20"
    }
  ]
}
```

Validation categories:

```text
package
manifest
markdown
tables
schemas
views
layout
page-map
render-consistency
integrity
security
```

Core validation rules:

```text
- mimetype exists and matches application/vnd.mcd+zip
- manifest exists and is valid JSON
- manifest format is MCD
- manifest version is supported
- entrypoint Markdown exists
- all declared table IDs are unique
- all table anchors resolve
- all table data files exist
- all table schema files exist
- CSV headers match schema columns
- CSV values conform to schema types
- table views reference existing columns
- placement refs are unique when present
```

---

## 12. Export layer

Crate module:

```text
mcd_core::export
```

The parser should generate views on demand. Do not store a separate `llm.md` file.

Export targets:

```text
canonical JSON document stream
Markdown with table anchors preserved
Markdown with tables expanded
plain text
CSV table dumps
schema summary
agent context JSON
layout summary JSON
```

CLI examples:

```bash
mcd extract report.mcd --json
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --tables
mcd extract report.mcd --layout
```

Python examples:

```python
doc.to_json()
doc.markdown(expand_tables=False)
doc.markdown(expand_tables=True)
doc.to_agent_context(include_tables=True, include_layout=False)
```

---

## 13. CLI stack

Crate:

```text
crates/mcd-cli
```

Recommended CLI dependency:

```text
clap
```

Commands:

```bash
mcd validate <file.mcd>
mcd inspect <file.mcd>
mcd extract <file.mcd> --json
mcd extract <file.mcd> --markdown
mcd extract <file.mcd> --markdown --expand-tables
mcd extract <file.mcd> --tables
mcd extract <file.mcd> --layout
mcd pack <directory> --output report.mcd
mcd unpack <file.mcd> --output directory
mcd init <directory>
mcd render <file.mcd> --html
mcd render <file.mcd> --pdf
```

CLI design rules:

```text
- stdout for extracted content
- stderr for diagnostics
- nonzero exit code for validation failure
- JSON output option for automation
- stable machine-readable diagnostics
- no network access by default
```

Example:

```bash
mcd validate annual-report.mcd --format json
```

Output:

```json
{
  "valid": true,
  "profile": "MCD-Core",
  "tables": 2,
  "tableAnchors": 2,
  "diagnostics": []
}
```

---

## 14. Python stack

Directory:

```text
bindings/python
```

Recommended tools:

```text
PyO3
maturin
pytest
ruff
mypy or pyright
pandas optional extra
```

Package name:

```text
mcd
```

Install:

```bash
pip install mcd
```

Optional data extra:

```bash
pip install "mcd[pandas]"
```

Recommended Python API:

```python
import mcd

doc = mcd.open("report.mcd")

result = doc.validate()
print(result.valid)

for block in doc.blocks():
    print(block.type, block.source)

table = doc.table("revenue")
print(table.schema)
print(table.rows())

# Optional pandas support
df = table.to_pandas()
```

Agent API:

```python
context = doc.to_agent_context(
    include_tables=True,
    include_layout=True,
)
```

Recommended Python classes:

```text
mcd.Document
mcd.Block
mcd.Table
mcd.TableSchema
mcd.TableView
mcd.Layout
mcd.ValidationResult
mcd.Diagnostic
```

Python binding rules:

```text
- Keep parsing in Rust.
- Keep validation in Rust.
- Convert Rust errors into Python exceptions.
- Return Python-native dict/list structures where useful.
- Make pandas optional.
- Do not require pandas for core use.
```

---

## 15. WebAssembly and TypeScript stack

Directories:

```text
crates/mcd-wasm
bindings/typescript
```

Recommended tools:

```text
wasm-bindgen
wasm-pack
TypeScript
Vitest
ESM package output
```

Package name:

```text
@mcd/parser
```

Target environments:

```text
browser
Node.js
Deno, optional later
```

Recommended TypeScript API:

```ts
import { openMcd } from "@mcd/parser";

const doc = await openMcd(fileBytes);

const validation = doc.validate();
const blocks = doc.blocks();
const markdown = doc.markdown({ expandTables: true });
const table = doc.table("revenue");
```

WASM restrictions:

```text
- no direct filesystem dependency
- accept bytes or ArrayBuffer
- no network access
- no native PDF rendering dependency
- expose pure parser/validator/export operations
```

---

## 16. Renderer stack

Rendering should be a separate layer from parsing.

```text
mcd-core parses and validates.
mcd-render renders.
```

MCD should support rendering, but MCD-Core should not depend on rendering.

## 16.1 Recommended v0.1 rendering path

Start with:

```text
MCD source
→ canonical document stream
→ HTML + CSS
→ optional PDF via HTML-to-PDF engine
→ optional page-map generation
```

Why HTML first:

```text
- easy browser preview
- easy debugging
- CSS-like layout maps naturally to styles.json
- PDF can be generated later from HTML
- supports progressive implementation
```

## 16.2 Renderer options

| Renderer option | Role | Recommendation |
|---|---|---|
| Rust HTML generator | Produce semantic HTML from MCD | Use in v0.1 |
| Static CSS | Human-facing page styles | Use in v0.1 |
| WeasyPrint | HTML/CSS to PDF | Good open-source option |
| PrinceXML | HTML/CSS to PDF | Good commercial/high-quality option |
| Headless Chromium | Browser-based PDF export | Useful for web-like rendering |
| Typst | Strong typesetting engine | Consider later, but avoid coupling v0.1 to it |

Recommended v0.1:

```text
Generate HTML first.
Generate PDF second.
Generate page-map after layout model stabilizes.
```

## 16.3 Page-map generation

The hard part is not PDF export; it is producing accurate `layout/page-map.json`.

Possible approaches:

```text
1. HTML-first renderer with DOM element IDs, then browser/layout engine returns bounding boxes.
2. Custom Rust layout engine for deterministic object positions.
3. PDF generation engine with exposed layout metadata.
```

Recommended initial approach:

```text
Use HTML elements with stable source IDs.
Use a browser or rendering engine to measure boxes.
Emit page-map from measured layout.
```

Long-term option:

```text
Build a dedicated deterministic layout engine if source-to-page verification becomes central.
```

---

## 17. Schema stack

MCD uses JSON for:

```text
manifest.json
table.schema.json
table.view.json
layout/styles.json
layout/page-map.json
integrity/checksums.json
```

Recommended schema approach:

```text
Use JSON Schema for validating the shape of MCD metadata files.
Use custom MCD table schema for table column definitions.
Use Rust typed structs for internal representation.
```

Schema files:

```text
schemas/manifest.schema.json
schemas/table.schema.json
schemas/table-view.schema.json
schemas/styles.schema.json
schemas/page-map.schema.json
```

Implementation:

```text
serde_json parses JSON.
jsonschema validates JSON structure.
MCD-specific validator enforces cross-file rules.
```

Important distinction:

```text
JSON Schema can validate individual files.
MCD validation must also validate relationships between files.
```

Example cross-file rules:

```text
- table anchors in Markdown must resolve to manifest table IDs
- table view columns must exist in table schema
- CSV headers must match schema columns
- page-map source references must point to Markdown/table objects
```

---

## 18. Table storage strategy

Use CSV as the canonical v0.1 table storage.

Reasons:

```text
- simple
- inspectable
- easy to generate
- widely supported
- easy for LLM tooling
- easy for Python, Rust, and JavaScript
```

Canonical v0.1:

```text
tables/<id>.csv
tables/<id>.schema.json
tables/<id>.view.json
```

Optional future additions:

```text
tables/<id>.arrow
tables/<id>.parquet
```

Future binary table formats should be optional accelerators, not required canonical storage in v0.1.

---

## 19. Integrity and signing stack

Integrity files:

```text
integrity/checksums.json
integrity/signatures.json
```

Recommended hashing:

```text
SHA-256 for file checksums
```

Recommended Rust crate:

```text
sha2
```

Checksums example:

```json
{
  "algorithm": "sha256",
  "files": {
    "manifest.json": "sha256:...",
    "content/main.md": "sha256:...",
    "tables/revenue.csv": "sha256:..."
  }
}
```

Digital signatures should come later, after the package and checksum model stabilizes.

Possible future signing approaches:

```text
- minisign-style detached signatures
- Sigstore/cosign-style signing
- JOSE/JWS-based signatures
- COSE-based signatures
```

Do not put signing into the MVP unless compliance use cases demand it.

---

## 20. Testing stack

Testing must cover both correctness and security.

## 20.1 Rust tests

Recommended tools:

```text
cargo test
insta snapshot tests
proptest property tests
cargo fuzz, later
cargo clippy
cargo fmt
```

Test categories:

```text
unit tests
integration tests
snapshot tests
fixture tests
invalid document tests
security archive tests
cross-platform path tests
conformance tests
```

## 20.2 Python tests

Recommended tools:

```text
pytest
ruff
mypy or pyright
```

Test categories:

```text
open document
validate document
read blocks
read tables
convert table to pandas
exception behavior
```

## 20.3 TypeScript tests

Recommended tools:

```text
Vitest
Playwright, only for viewer tests
```

Test categories:

```text
load WASM parser
parse bytes
validate fixtures
export Markdown
export JSON
```

## 20.4 Conformance suite

Create official test fixtures:

```text
tests/fixtures/conformance/
  valid-minimal.mcd
  valid-table.mcd
  valid-two-tables.mcd
  valid-reused-table.mcd
  invalid-missing-manifest.mcd
  invalid-bad-mimetype.mcd
  invalid-unresolved-table.mcd
  invalid-csv-header-mismatch.mcd
  invalid-nonnullable-empty-cell.mcd
  invalid-path-traversal.mcd
```

Every implementation must pass the conformance suite.

---

## 21. Build and release stack

## 21.1 Rust release

Use:

```text
cargo
cargo-release, optional
GitHub Actions
```

Publish:

```text
mcd-core to crates.io
mcd-cli to GitHub Releases as binaries
```

CLI binary targets:

```text
linux x86_64
linux aarch64
macOS x86_64
macOS arm64
Windows x86_64
```

## 21.2 Python release

Use:

```text
maturin
PyPI
manylinux wheels
macOS wheels
Windows wheels
```

Recommended build command:

```bash
maturin build --release
```

Recommended publish command:

```bash
maturin publish
```

## 21.3 TypeScript release

Use:

```text
wasm-pack
npm
TypeScript declarations
ESM-first package
```

Package output:

```text
@mcd/parser
  dist/
    index.js
    index.d.ts
    mcd_wasm_bg.wasm
```

---

## 22. CI/CD stack

Recommended GitHub Actions workflows:

```text
ci.yml
  cargo fmt
  cargo clippy
  cargo test
  pytest
  npm test

security.yml
  cargo audit
  dependency review
  test malicious ZIP fixtures

release-rust.yml
  build mcd-cli binaries
  publish crates

release-python.yml
  build wheels with maturin
  publish to PyPI

release-npm.yml
  build WASM package
  publish to npm
```

Minimum CI checks:

```text
- Rust formatting passes
- Rust clippy passes
- Rust tests pass
- Python tests pass
- TypeScript tests pass
- conformance fixtures pass
- malicious archive tests pass
```

---

## 23. Documentation stack

Documentation should exist at three levels.

## 23.1 Specification docs

```text
SPEC.md
  Format structure, rules, schemas, validation, conformance.

ABOUT.md
  Conceptual overview.

TECH_STACK.md
  Implementation stack.
```

## 23.2 Developer docs

```text
docs/parser-api.md
docs/cli.md
docs/python.md
docs/typescript.md
docs/validation.md
docs/rendering.md
```

## 23.3 API docs

```text
Rust: docs.rs
Python: generated API docs or mkdocs
TypeScript: typedoc
```

Recommended docs site later:

```text
MkDocs Material
```

---

## 24. Security model implementation

MCD parsers will process files from users. Treat every `.mcd` as untrusted input.

Required protections:

```text
- archive path traversal protection
- decompression bomb limits
- file count limits
- file size limits
- UTF-8 validation where required
- duplicate normalized path rejection
- strict JSON parsing
- strict CSV schema validation
- no script execution
- no remote resource loading
```

Recommended implementation settings:

```text
MAX_PACKAGE_FILES = configurable
MAX_COMPRESSED_SIZE = configurable
MAX_DECOMPRESSED_SIZE = configurable
MAX_MARKDOWN_SIZE = configurable
MAX_TABLE_ROWS = configurable
MAX_TABLE_COLUMNS = configurable
MAX_CELL_BYTES = configurable
```

All limits should have safe defaults and configurable overrides.

---

## 25. Error model

Errors must be structured and stable.

Bad:

```text
"Something went wrong."
```

Good:

```json
{
  "code": "csv.header.mismatch",
  "message": "CSV header `revenue` does not match schema column `revenue_gbp`.",
  "severity": "error",
  "source": "tables/revenue.csv:1",
  "related": ["tables/revenue.schema.json:12"]
}
```

Recommended error code namespaces:

```text
package.*
manifest.*
markdown.*
directive.*
table.*
csv.*
schema.*
view.*
layout.*
page_map.*
render.*
integrity.*
security.*
```

---

## 26. MVP implementation plan

## Phase 0: Spec skeleton

Deliverables:

```text
ABOUT.md
TECH_STACK.md
SPEC.md draft
manifest.schema.json draft
table.schema.json draft
example unpacked MCD document
```

## Phase 1: Core package and manifest parser

Deliverables:

```text
mcd-core::package
mcd-core::manifest
open package
read mimetype
read manifest
validate manifest basics
```

CLI:

```bash
mcd inspect example.mcd
```

## Phase 2: Markdown and table anchors

Deliverables:

```text
parse content/main.md
identify table directives
produce document block stream
report source locations
```

CLI:

```bash
mcd extract example.mcd --json
```

## Phase 3: CSV + schema validation

Deliverables:

```text
load table CSV
load table schema
coerce typed values
validate rows
resolve table anchors
```

CLI:

```bash
mcd validate example.mcd
mcd extract example.mcd --tables
```

## Phase 4: Expanded Markdown export

Deliverables:

```text
render table refs as Markdown tables
apply table views
format currency/percent/date values
```

CLI:

```bash
mcd extract example.mcd --markdown --expand-tables
```

## Phase 5: Python bindings

Deliverables:

```text
mcd.open()
Document.validate()
Document.blocks()
Document.table()
Document.markdown(expand_tables=True)
Table.rows()
Table.schema
```

Install:

```bash
pip install mcd
```

## Phase 6: HTML renderer

Deliverables:

```text
MCD → semantic HTML
MCD styles → CSS
HTML preview
```

CLI:

```bash
mcd render example.mcd --html
```

## Phase 7: PDF export and page map

Deliverables:

```text
HTML/CSS → PDF
page-map.json
source-to-render object IDs
```

CLI:

```bash
mcd render example.mcd --pdf --page-map
```

## Phase 8: WASM/TypeScript

Deliverables:

```text
@mcd/parser
openMcd(bytes)
doc.blocks()
doc.validate()
doc.markdown({ expandTables: true })
```

---

## 27. MVP dependency set

For the first usable version, keep dependencies small.

Recommended MVP Rust dependencies:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
zip = "*"
csv = "1"
comrak = "*"
thiserror = "1"
anyhow = "1"
clap = { version = "4", features = ["derive"] }
indexmap = { version = "*", features = ["serde"] }
sha2 = "*"
rust_decimal = { version = "*", features = ["serde"] }
time = { version = "*", features = ["serde", "parsing", "formatting"] }
```

Notes:

```text
- Pin exact versions once implementation starts.
- Do not leave wildcard versions in released crates.
- The wildcard list above is a planning sketch, not production Cargo.toml.
```

---

## 28. Parser API sketch

## 28.1 Rust API

```rust
use mcd_core::Document;

fn main() -> mcd_core::Result<()> {
    let doc = Document::open("annual-report.mcd")?;

    let validation = doc.validate()?;
    if !validation.is_valid() {
        for diagnostic in validation.diagnostics() {
            eprintln!("{}: {}", diagnostic.code(), diagnostic.message());
        }
    }

    for block in doc.blocks()? {
        println!("{:?}", block);
    }

    let revenue = doc.table("revenue")?;
    println!("{:?}", revenue.schema());
    println!("{:?}", revenue.rows());

    Ok(())
}
```

## 28.2 Python API

```python
import mcd

doc = mcd.open("annual-report.mcd")

validation = doc.validate()
assert validation.valid

for block in doc.blocks():
    print(block.type, block.source)

revenue = doc.table("revenue")
print(revenue.schema)
print(revenue.rows())

print(doc.markdown(expand_tables=True))
```

## 28.3 CLI API

```bash
mcd validate annual-report.mcd
mcd extract annual-report.mcd --json
mcd extract annual-report.mcd --markdown --expand-tables
mcd extract annual-report.mcd --tables revenue
```

## 28.4 TypeScript API

```ts
import { openMcd } from "@mcd/parser";

const doc = await openMcd(bytes);

const validation = doc.validate();
const markdown = doc.markdown({ expandTables: true });
const revenue = doc.table("revenue");
```

---

## 29. Renderer API sketch

Renderer input:

```text
Document stream
Resolved tables
Table views
Layout styles
```

Renderer output:

```text
HTML
CSS
PDF, optional
page-map.json, optional
```

Rust API:

```rust
let doc = Document::open("annual-report.mcd")?;
let html = mcd_render::to_html(&doc)?;
let pdf = mcd_render::to_pdf(&doc)?;
let page_map = mcd_render::page_map(&doc)?;
```

CLI:

```bash
mcd render annual-report.mcd --html --output report.html
mcd render annual-report.mcd --pdf --output report.pdf
mcd render annual-report.mcd --page-map --output page-map.json
```

---

## 30. What not to build first

Do not build these in v0.1:

```text
- full custom layout engine
- digital signatures
- interactive widgets
- JavaScript rendering
- remote resources
- binary-only table storage
- DOCX import/export
- PDF reverse parser
- AI-specific prompting logic
- domain-specific taxonomies
- chart grammar beyond table-backed views
```

Build the parser and validator first.

---

## 31. Recommended first release scope

First public alpha:

```text
mcd-core
mcd-cli
mcd-python
examples
schemas
conformance fixtures
```

Supported features:

```text
- package open/read
- manifest validation
- Markdown parsing
- table anchors
- CSV loading
- table schema validation
- table view loading
- document stream export
- expanded Markdown export
- Python API
- CLI validation/extraction
```

Not required for first alpha:

```text
- PDF rendering
- page-map validation
- WASM package
- signatures
```

---

## 32. Recommended naming

Project:

```text
MCD
Markdown CSV Document
```

File extension:

```text
.mcd
```

MIME type:

```text
application/vnd.mcd+zip
```

Rust crates:

```text
mcd-core
mcd-cli
mcd-render
mcd-wasm
```

Python package:

```text
mcd
```

TypeScript package:

```text
@mcd/parser
```

CLI executable:

```text
mcd
```

---

## 33. Technical principles

```text
Canonical content is Markdown + CSV + JSON metadata.
The parser never guesses tables from layout.
The renderer never becomes the source of truth.
The PDF is an export, not canonical.
The Python API wraps the Rust parser.
The TypeScript API wraps the same Rust parser via WASM.
All conformance checks live in mcd-core.
All file paths are validated before access.
All extracted output can be generated from canonical files.
No stored llm.md is required.
```

---

## 34. Final stack summary

```text
Core:
  Rust

Package:
  ZIP-like .mcd container
  mimetype
  manifest.json

Prose:
  CommonMark-compatible Markdown
  MCD table directives

Tables:
  CSV
  JSON table schema
  JSON table view

Metadata:
  JSON
  JSON Schema for metadata validation

Parser:
  mcd-core Rust crate

CLI:
  mcd-cli Rust binary using clap

Python:
  PyO3 + maturin
  package name: mcd

Web:
  Rust → WebAssembly
  TypeScript wrapper
  package name: @mcd/parser

Renderer:
  HTML/CSS first
  PDF export second
  page-map after layout stabilizes

Validation:
  mcd-core conformance engine
  JSON Schema + cross-file validation

Testing:
  cargo test
  snapshot fixtures
  conformance suite
  pytest
  Vitest

Release:
  crates.io
  PyPI
  npm
  GitHub Releases
```

This stack gives MCD a realistic path from specification to usable parser, Python library, CLI validator, and eventually browser-compatible tooling without duplicating parsing logic across ecosystems.

---

## 35. Reference links

These are implementation references, not mandatory dependencies:

- [Rust](https://www.rust-lang.org/)
- [CommonMark](https://commonmark.org/)
- [Comrak](https://github.com/kivikakk/comrak)
- [Serde](https://serde.rs/)
- [csv crate](https://docs.rs/csv)
- [jsonschema crate](https://docs.rs/jsonschema)
- [zip crate](https://docs.rs/zip)
- [clap crate](https://docs.rs/clap)
- [PyO3](https://pyo3.rs/)
- [maturin](https://www.maturin.rs/)
- [wasm-pack](https://rustwasm.github.io/docs/wasm-pack/)
- [WeasyPrint](https://weasyprint.org/)
- [PrinceXML](https://www.princexml.com/)
- [Apache Arrow](https://arrow.apache.org/docs/format/Columnar.html)
