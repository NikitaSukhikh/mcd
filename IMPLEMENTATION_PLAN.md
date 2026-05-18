# MCD Implementation Plan

This plan converts the current MCD documentation into an executable build sequence.
It assumes the repository currently contains specification and planning documents only,
and that the first target is a usable alpha parser, validator, CLI, and Python API.

## Guiding decisions

- Build one canonical parser in Rust and expose it through CLI, Python, and later WASM.
- Treat `.mcd` files as untrusted ZIP-like packages.
- Make `mcd-core` the owner of all conformance, parsing, validation, and export logic.
- Keep rendering separate from parsing.
- Support MCD-Core before MCD-Rendered, MCD-Verified, or MCD-Signed.
- Generate expanded Markdown and agent context on demand; do not store `llm.md`.
- Keep v0.1 table storage as CSV plus JSON schema and optional JSON view.
- Treat visual content with one rule: anything visual is allowed, but anything meaningful must also be represented as Markdown text, typed table data, or machine-readable layout/view metadata.
- Store images as asset-backed metadata objects, not as semantic content.
- Treat charts as rendered views of typed CSV tables, not as standalone image objects.
- Prefer SVG for diagrams and generated chart assets, while disallowing scripts, active behavior, and external resource loading inside SVG.
- Use Apache-2.0 for code, CC-BY-4.0 for docs/specs, and CC0-1.0 for schemas, examples, and fixtures.

## Target alpha scope

The first public alpha should include:

- Rust workspace with `mcd-core` and `mcd-cli`.
- Package reader for `.mcd` archives.
- Manifest parser and basic manifest validation.
- Markdown parser with MCD table directive detection.
- Markdown parser with MCD image directive detection.
- CSV table loader with schema validation.
- Table view loader and view-column validation.
- Chart view validation through table views with `display: chart`.
- Image metadata loader and asset validation.
- Canonical document block stream.
- Expanded Markdown export.
- JSON extraction export.
- Structured diagnostics.
- CLI commands for `inspect`, `validate`, `extract`, `pack`, `unpack`, and `init`.
- Python bindings exposing the core parser.
- JSON schemas for manifest, table schema, table view, image metadata, generated renderings, styles, and page map.
- Minimal, table-backed, image-backed, and chart-backed examples.
- Valid, invalid, and security-focused conformance fixtures.

Out of scope for the first alpha:

- PDF rendering.
- Page-map generation and render/source verification.
- Digital signatures.
- WASM and TypeScript package.
- Custom layout engine.
- Remote resources.
- Domain-specific taxonomies.
- OCR or automated extraction of text from image pixels.
- General-purpose Vega-Lite compatibility.
- Chart rendering verification against generated SVG/PDF output.

## Visual content model

MCD keeps visual material narrow and machine-readable:

- Text lives in Markdown.
- Numbers and meaningful tables live in CSV plus schema.
- Charts are table views.
- Images are visual assets with metadata.
- Layout and rendered positions live in JSON.

Conformance levels:

```text
MCD-Core
  Markdown text, typed tables, layout, and page-map references.

MCD-Images
  Images allowed with declared role, alt text, caption, dimensions, media type, and hash.

MCD-Charts
  Charts allowed only as table-backed views.

MCD-Strict
  No meaningful text, table, or numeric content may exist only inside an image.
```

Fully machine-readable visual documents should claim:

```json
{
  "conformance": [
    "MCD-Core",
    "MCD-Images",
    "MCD-Charts",
    "MCD-Strict"
  ]
}
```

The first alpha should validate declared metadata and cross-file references. It should not attempt OCR or pixel-based semantic inference.

## Repository setup

Create this initial structure:

```text
mcd/
  Cargo.toml
  LICENSE
  NOTICE
  README.md
  ABOUT.md
  MCD_TECH_STACK.md
  DEPENDENCIES.md
  LICENSING.md
  IMPLEMENTATION_PLAN.md

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
        assets.rs
        images.rs
        document.rs
        validate.rs
        export.rs
        errors.rs

    mcd-cli/
      Cargo.toml
      src/
        main.rs
        commands/
          inspect.rs
          validate.rs
          extract.rs
          pack.rs
          unpack.rs
          init.rs

  bindings/
    python/
      pyproject.toml
      Cargo.toml
      src/
        lib.rs
      mcd/
        __init__.py
        py.typed

  schemas/
    manifest.schema.json
    table.schema.json
    table-view.schema.json
    image.schema.json
    rendering.schema.json
    styles.schema.json
    page-map.schema.json

  examples/
    minimal/
      unpacked/
        mimetype
        manifest.json
        content/
          main.md
    revenue-report/
      unpacked/
        mimetype
        manifest.json
        content/
          main.md
        tables/
          revenue.csv
          revenue.schema.json
          revenue.view.json
    visual-report/
      unpacked/
        mimetype
        manifest.json
        content/
          main.md
        assets/
          process-diagram.svg
          factory-photo.jpg
        images/
          process-diagram.image.json
          factory-photo.image.json
    chart-report/
      unpacked/
        mimetype
        manifest.json
        content/
          main.md
        tables/
          revenue.csv
          revenue.schema.json
          revenue.chart.view.json
        rendered/
          revenue-chart.svg
          revenue-chart.rendering.json

  tests/
    fixtures/
      valid/
      invalid/
      security/
      conformance/
```

## Phase 0: Project foundation - Completed 2026-05-18

Status:

- [x] Workspace, `mcd-core`, and `mcd-cli` created.
- [x] Apache-2.0 `LICENSE` already present and `NOTICE` added.
- [x] License notes for docs, schemas, examples, and fixtures added to `NOTICE`.
- [x] Formatting and placeholder tests pass locally.
- [x] `mcd --help` runs.
- [ ] Minimal CI workflow deferred until a remote repository is available.

Deliverables:

- Add `LICENSE` with Apache License 2.0.
- Add `NOTICE`.
- Add clear license notes for docs, schemas, examples, and fixtures.
- Add Rust workspace root `Cargo.toml`.
- Add `mcd-core` and `mcd-cli` crates.
- Add formatting and lint defaults.
- Add a minimal CI workflow once a remote repository is available.

Initial Rust core dependencies to start:

```toml
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

CLI dependencies:

```toml
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

Acceptance criteria:

- `cargo fmt` passes.
- `cargo test` passes with placeholder crate tests.
- `mcd --help` runs.
- Wildcard dependency versions are tracked as temporary and pinned before release.
- `DEPENDENCIES.md` remains the full project-wide dependency inventory; this phase lists only the dependencies needed to start the Rust parser, validator, CLI, and tests.

## Phase 1: Package reader and manifest parser - Completed 2026-05-18

Status:

- [x] `mcd_core::package::McdPackage` implemented.
- [x] Safe archive opening from file path and byte slice implemented.
- [x] Root `mimetype` read and validation implemented.
- [x] Safe internal path handling implemented.
- [x] Manifest file read implemented.
- [x] `mcd_core::manifest::Manifest` and related structs implemented.
- [x] Basic manifest validation implemented.
- [x] Required archive protections covered by unit tests.
- [x] `mcd inspect examples/minimal/minimal.mcd` runs against a real packed example.

Implement:

- `mcd_core::package::McdPackage`.
- Safe archive opening from file path and byte slice.
- Root `mimetype` read and validation.
- Safe internal path handling.
- Manifest file read.
- `mcd_core::manifest::Manifest` and related structs.
- Basic manifest validation.

Required archive protections:

- Reject absolute paths.
- Reject `..` traversal.
- Reject duplicate normalized paths.
- Reject unsafe path separators or ambiguous normalized paths.
- Enforce maximum file count.
- Enforce maximum single-file size.
- Enforce maximum total decompressed size.
- Never extract automatically during normal parsing.

CLI:

```bash
mcd inspect examples/minimal/minimal.mcd
```

Acceptance criteria:

- Valid minimal package can be inspected.
- Missing `mimetype` fails with a structured diagnostic.
- Bad `mimetype` fails.
- Missing `manifest.json` fails.
- Path traversal fixture fails.
- Duplicate normalized path fixture fails.

## Phase 2: Markdown parser and table directives - Completed 2026-05-18

Status:

- [x] `mcd_core::markdown` implemented.
- [x] `mcd_core::directives` implemented.
- [x] CommonMark-compatible Markdown parsing through `comrak` implemented.
- [x] Block table directive detection implemented.
- [x] Chart placement detection through `:::table` with `display: chart` implemented.
- [x] Block image directive detection implemented.
- [x] Canonical block stream types implemented for headings, paragraphs, lists, code blocks, quotes, math blocks, table refs, and image refs.
- [x] Source spans included where available.
- [x] Stable generated block IDs implemented.
- [x] Directive validation rules implemented for required fields, display values, chart view requirement, unsupported `:::chart`, duplicate placement refs, and strict unknown fields.
- [x] `mcd extract examples/revenue-report/revenue-report.mcd --json` runs against a real packed example.

Implement:

- `mcd_core::markdown`.
- `mcd_core::directives`.
- CommonMark-compatible Markdown parsing through `comrak`.
- Detection of block table directives:

```markdown
:::table
ref: revenue-table
table: revenue
view: default
display: table
caption: Revenue by quarter
numbering: auto
:::
```

- Detection of chart placements through the same table directive family:

```markdown
:::table
ref: revenue-chart
table: revenue
view: quarterly-bar-chart
display: chart
caption: Revenue by quarter
:::
```

- Detection of block image directives:

```markdown
:::image
ref: process-diagram
asset: process-diagram
caption: Facility layout showing intake, processing, quality control, and dispatch.
alt: Diagram of the facility workflow from intake to dispatch.
:::
```

- Canonical block stream types:

```text
heading
paragraph
list
code_block
quote
math_block
table_ref
image_ref
```

- Source spans where available.
- Stable generated block IDs.

Validation rules:

- Every table directive must include `table`.
- Table directive `display` must be `table` or `chart`; omitted `display` defaults to `table`.
- Table directives with `display: chart` must include `view`.
- No separate `:::chart` directive is supported in v0.1.
- Every image directive must include `asset` or a resolvable image metadata reference.
- Placement `ref` values must be unique when present.
- Directive fields must be known in strict mode.
- Markdown pipe tables are not canonical semantic tables in MCD-Core.

CLI:

```bash
mcd extract examples/revenue-report/revenue-report.mcd --json
```

Acceptance criteria:

- Markdown blocks are emitted in source order.
- Table anchors appear exactly where declared.
- Chart anchors appear as table placements with `display: chart`.
- Image anchors appear exactly where declared.
- Invalid table directive syntax produces a structured diagnostic.
- Invalid image directive syntax produces a structured diagnostic.
- Duplicate placement refs fail validation.

## Phase 3: Table data, schema, and view validation - Completed 2026-05-18

Status:

- [x] `mcd_core::tables` implemented.
- [x] `mcd_core::schema` implemented.
- [x] `mcd_core::table_view` implemented.
- [x] CSV loading from manifest-declared paths implemented.
- [x] MCD table schema JSON parsing implemented.
- [x] Table and chart view JSON parsing implemented.
- [x] Typed value coercion implemented for string, integer, decimal, boolean, date, datetime, time, and enum values.
- [x] Cross-file validation implemented between manifest, Markdown anchors, CSV files, schemas, and views.
- [x] Chart view validation implemented for the alpha chart subset.
- [x] `mcd validate examples/revenue-report/revenue-report.mcd` runs against the packed revenue example.
- [x] `mcd extract examples/revenue-report/revenue-report.mcd --tables` emits typed table data.

Implement:

- `mcd_core::tables`.
- `mcd_core::schema`.
- `mcd_core::table_view`.
- CSV loading from manifest-declared paths.
- JSON schema file parsing for MCD table schemas.
- JSON table view parsing.
- JSON chart view parsing as a constrained table view form.
- Typed value coercion.
- Cross-file validation between manifest, Markdown anchors, CSV, schemas, and views.

Supported primitive types:

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

Validation rules:

- Manifest table IDs are unique.
- Every table anchor resolves to a declared table ID.
- Every declared table data file exists.
- Every declared table schema file exists.
- CSV header row exists.
- CSV headers match schema column names.
- CSV rows conform to declared column types.
- Empty cells are allowed only for nullable columns.
- Enum values are members of the declared enum.
- View columns reference schema columns.
- View IDs referenced by anchors exist when declared.
- Chart view IDs referenced by `display: chart` anchors exist.
- Chart view `table` matches the anchor's table.
- Chart axis, series, grouping, and mark-label columns reference schema columns.
- Chart columns use compatible schema types.
- Chart numeric encodings use `integer` or `decimal` columns.
- Chart temporal encodings use `date`, `datetime`, or `time` columns.
- Chart labels, formatting, currency, unit, and percent declarations are consistent with the table schema and view.
- Rendered chart assets, when declared, are marked as generated renderings and identify their source table and view.

Supported alpha chart view subset:

```text
bar
line
area
scatter
```

Chart view example:

```json
{
  "id": "quarterly-bar-chart",
  "table": "revenue",
  "display": "chart",
  "chart": {
    "type": "bar",
    "x": {
      "column": "quarter",
      "label": "Quarter"
    },
    "y": {
      "column": "revenue_gbp",
      "label": "Revenue",
      "format": "currency",
      "currency": "GBP"
    },
    "markLabels": {
      "show": true,
      "format": "currency",
      "currency": "GBP"
    }
  },
  "style": {
    "width": "160mm",
    "height": "90mm"
  }
}
```

CLI:

```bash
mcd validate examples/revenue-report/revenue-report.mcd
mcd extract examples/revenue-report/revenue-report.mcd --tables
```

Acceptance criteria:

- Valid revenue example passes validation.
- Header mismatch fixture fails.
- Non-nullable empty cell fixture fails.
- Bad decimal, date, datetime, time, boolean, and enum fixtures fail.
- Unresolved table anchor fixture fails.
- Unknown view-column fixture fails.
- Unknown chart-column fixture fails.
- Chart view with incompatible column types fails.
- Chart anchor referencing a non-chart view fails.

## Phase 3A: Images and visual asset validation - Completed 2026-05-18

Status:

- [x] `mcd_core::assets` implemented.
- [x] `mcd_core::images` implemented.
- [x] Manifest image metadata entries and optional asset path entries implemented.
- [x] Image metadata loading from `images/*.image.json` implemented.
- [x] Image directive resolution from Markdown anchors to declared image metadata implemented.
- [x] Media type detection and declared media type validation implemented.
- [x] SHA-256 hash validation for referenced image assets implemented.
- [x] Intrinsic size validation implemented.
- [x] SVG safety validation implemented for scripts, event handlers, active animation, external references, and foreign content.
- [x] Image role, alt text, caption, decorative image, meaningful content, and MCD-Strict validation rules implemented.
- [x] `examples/visual-report/visual-report.mcd` added and validates.
- [x] `mcd extract examples/visual-report/visual-report.mcd --json` emits image metadata.
- [x] `mcd extract examples/visual-report/visual-report.mcd --images` emits image metadata without binary assets.

Implement:

- `mcd_core::assets`.
- `mcd_core::images`.
- Asset entries in the manifest.
- Image metadata loading from `images/*.image.json`.
- Image directive resolution from Markdown anchors to image metadata and assets.
- Media type detection and declared media type validation.
- Hash validation for referenced image assets.
- Intrinsic size metadata validation when declared.
- SVG safety validation for scripts, event handlers, active behavior, and external resource references.

Image metadata example:

```json
{
  "id": "process-diagram",
  "asset": "assets/process-diagram.svg",
  "mediaType": "image/svg+xml",
  "role": "diagram",
  "caption": "Facility layout showing intake, processing, quality control, and dispatch.",
  "alt": "Diagram of the facility workflow from intake to dispatch.",
  "intrinsicSize": {
    "width": 1200,
    "height": 800,
    "unit": "px"
  },
  "hash": "sha256:..."
}
```

Supported image roles:

```text
decorative
informative
diagram
photo
logo
rendered-table-prohibited
rendered-text-prohibited
```

Validation rules:

- Manifest image IDs are unique.
- Every image anchor resolves to a declared image ID or asset-backed image metadata object.
- Every image metadata object references an existing asset.
- Every image asset is inside `assets/` or another manifest-declared asset directory.
- Informative, diagram, photo, and logo images require non-empty `alt`.
- Informative and diagram images require a caption.
- Decorative images must have empty `alt` unless explicitly overridden by accessibility metadata.
- `rendered-table-prohibited` and `rendered-text-prohibited` roles are invalid in MCD-Strict packages.
- If metadata declares that an image contains meaningful text, numbers, or table-like data, the metadata must link to canonical Markdown or table references.
- SVG assets must not contain scripts, event handler attributes, active animation behavior, external network references, or embedded foreign content.
- Raster image formats are allowed for photos and visual evidence, but not as canonical sources for tables or numbers.

CLI:

```bash
mcd validate examples/visual-report/visual-report.mcd
mcd extract examples/visual-report/visual-report.mcd --json
```

Acceptance criteria:

- Valid visual example passes validation.
- Missing image asset fixture fails.
- Missing informative image alt text fixture fails.
- Decorative image with required text alternative fixture behavior is covered.
- SVG script fixture fails.
- External SVG resource fixture fails.
- Image-only table fixture fails under MCD-Strict.

## Phase 4: Exports - Completed 2026-05-18

Status:

- [x] `mcd_core::document` implemented.
- [x] `mcd_core::export` implemented.
- [x] Canonical JSON document stream export implemented with tables, views, images, and chart placements.
- [x] Original Markdown export implemented.
- [x] Expanded Markdown export implemented with resolved table views.
- [x] Expanded Markdown export for chart placements implemented as chart metadata plus source-data tables.
- [x] Image metadata export implemented without embedding binary assets.
- [x] Chart metadata export implemented with exact source table and view references.
- [x] Table extraction export implemented.
- [x] Schema summary export API implemented.
- [x] Agent context JSON export API implemented.
- [x] `mcd extract` supports `--json`, `--markdown`, `--markdown --expand-tables`, `--tables`, `--images`, and `--charts`.

Implement:

- `mcd_core::document`.
- `mcd_core::export`.
- Canonical JSON document stream export.
- Original Markdown export.
- Expanded Markdown export with resolved table views.
- Expanded Markdown export for chart placements as source-data tables plus chart metadata.
- Image metadata export.
- Chart metadata export with exact source table and view references.
- Table extraction export.
- Schema summary export.
- Agent context JSON export.

Expanded Markdown table formatting should use:

- Schema labels when no view label exists.
- View labels when present.
- Raw typed values formatted according to view rules.
- Currency, percent, number, date, datetime, and string formatting.
- Alignment markers for Markdown table columns.

Chart export rules:

- `display: chart` placements remain table-backed in JSON.
- Expanded Markdown may include the chart caption followed by the exact source data as a Markdown table.
- Agent context must expose `table_id`, `view_id`, chart encodings, style metadata, and source layout references when available.
- Agents must never need to infer chart values from pixels.

Image export rules:

- Expanded Markdown includes image caption and alt text.
- JSON export includes image role, media type, intrinsic size, hash, and asset path.
- Agent context marks decorative images as non-semantic.
- Agent context includes canonical Markdown/table references for meaningful image content when declared.

CLI:

```bash
mcd extract report.mcd --json
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --tables
mcd extract report.mcd --images
mcd extract report.mcd --charts
```

Acceptance criteria:

- Expanded Markdown is generated from canonical Markdown, CSV, schemas, and views.
- No generated `llm.md` is written into the package.
- Snapshot tests cover JSON and expanded Markdown output.
- Output ordering is deterministic.
- Chart extraction returns exact table-backed data and view metadata.
- Image extraction returns metadata without embedding large binary assets by default.

## Phase 5: CLI completion

Implement stable CLI behavior:

```bash
mcd inspect <file.mcd>
mcd validate <file.mcd>
mcd validate <file.mcd> --format json
mcd extract <file.mcd> --json
mcd extract <file.mcd> --markdown
mcd extract <file.mcd> --markdown --expand-tables
mcd extract <file.mcd> --tables
mcd extract <file.mcd> --images
mcd extract <file.mcd> --charts
mcd pack <directory> --output <file.mcd>
mcd unpack <file.mcd> --output <directory>
mcd init <directory>
```

Rules:

- Extracted content goes to stdout by default.
- Diagnostics go to stderr by default.
- Validation failure exits nonzero.
- `--format json` emits stable machine-readable diagnostics.
- `pack` writes `mimetype` in the correct root position if possible.
- `unpack` refuses unsafe archive entries.
- `extract --images` emits image metadata and asset references, not binary blobs.
- `extract --charts` emits chart placements with their source table IDs, view IDs, and typed source rows.

Acceptance criteria:

- CLI commands have tests using fixtures.
- JSON diagnostic shape is stable.
- Exit codes are covered by tests.
- `mcd init`, `mcd pack`, and `mcd validate` can create and validate a minimal document.

## Phase 6: Python bindings

Implement:

- `bindings/python` using PyO3 and maturin.
- Python package name `mcd`.
- Rust-backed Python API:

```python
import mcd

doc = mcd.open("report.mcd")
validation = doc.validate()
blocks = doc.blocks()
table = doc.table("revenue")
chart = doc.chart("revenue-chart")
image = doc.image("process-diagram")
markdown = doc.markdown(expand_tables=True)
context = doc.to_agent_context(include_tables=True, include_layout=False)
```

Required Python classes:

```text
Document
Block
Table
TableSchema
TableView
Chart
Image
ValidationResult
Diagnostic
```

Chart API:

```python
chart.table_id
chart.view_id
chart.dataframe()
chart.to_markdown_table()
chart.layout()
```

Image API:

```python
image.asset_path
image.role
image.alt
image.caption
image.intrinsic_size
```

Rules:

- Keep parsing and validation in Rust.
- Convert Rust diagnostics into Python-native objects.
- Convert fatal Rust errors into Python exceptions.
- Keep pandas optional.

Acceptance criteria:

- `maturin develop` works locally.
- `pytest` covers open, validate, blocks, tables, expanded Markdown, and exceptions.
- `pytest` covers image metadata access and chart source-data extraction.
- Optional pandas extra can convert a table to a DataFrame when pandas is installed.
- Optional pandas extra can convert chart source data to a DataFrame when pandas is installed.

## Phase 7: JSON schemas and conformance fixtures

Implement:

- `schemas/manifest.schema.json`.
- `schemas/table.schema.json`.
- `schemas/table-view.schema.json`.
- `schemas/image.schema.json`.
- `schemas/rendering.schema.json`.
- `schemas/styles.schema.json`.
- `schemas/page-map.schema.json`.
- Conformance fixture set.

Schema rules:

- `table-view.schema.json` supports both table views and constrained chart views.
- Chart views use `display: chart` and a required `chart` object.
- Table views use `display: table` or omit `display`.
- `image.schema.json` validates role, alt text, caption, media type, intrinsic size, hash, and canonical source references for meaningful visual content.
- `rendering.schema.json` validates generated asset provenance, source table/view references, media type, and hash.
- Manifest schema supports `conformance` claims including `MCD-Core`, `MCD-Images`, `MCD-Charts`, and `MCD-Strict`.

Minimum fixtures:

```text
valid-minimal.mcd
valid-table.mcd
valid-two-tables.mcd
valid-reused-table.mcd
valid-image.mcd
valid-chart.mcd
valid-image-and-chart.mcd
invalid-missing-manifest.mcd
invalid-bad-mimetype.mcd
invalid-unresolved-table.mcd
invalid-csv-header-mismatch.mcd
invalid-nonnullable-empty-cell.mcd
invalid-unresolved-image.mcd
invalid-image-missing-alt.mcd
invalid-svg-script.mcd
invalid-image-only-table-strict.mcd
invalid-chart-unknown-column.mcd
invalid-chart-bad-type.mcd
invalid-path-traversal.mcd
```

Acceptance criteria:

- All fixtures are covered in automated tests.
- Fixture expected diagnostics are snapshot-tested.
- Schemas, examples, and fixtures carry CC0-1.0 notices.

## Phase 8: HTML renderer

Implement after parser and validator stabilize:

- `crates/mcd-render`.
- Semantic HTML output from the canonical document stream.
- CSS generation from a small subset of `layout/styles.json`.
- HTML image rendering from validated image metadata.
- HTML chart rendering from typed table data and chart view JSON.
- CLI support:

```bash
mcd render report.mcd --html --output report.html
```

Rules:

- Renderer depends on `mcd-core`.
- `mcd-core` does not depend on renderer.
- HTML contains stable source IDs for future page-map work.
- Meaningful text and tables still originate from Markdown and CSV.
- Rendered charts are generated from table values and chart view rules.
- Generated chart SVG can be written to `rendered/`, but remains non-canonical.
- Rendered images must use package assets and must not load remote resources.

Acceptance criteria:

- Valid table example renders to standalone HTML.
- Valid image example renders accessible image markup.
- Valid chart example renders a chart generated from CSV data.
- Table views affect visible column labels, ordering, formatting, and alignment.
- Chart views affect chart type, axes, labels, formatting, and colors.
- Renderer tests compare stable HTML snapshots.

## Phase 9: PDF export and page map

Implement only after HTML rendering is useful:

- HTML-to-PDF export through an external renderer or browser-based path.
- Optional `layout/page-map.json` generation.
- Source-to-render object IDs.
- Page-map entries for image and chart placements.
- Optional generated chart asset integrity checks.
- CLI support:

```bash
mcd render report.mcd --pdf --output report.pdf
mcd render report.mcd --html --page-map --output report.html
```

Acceptance criteria:

- Rendered tables map back to table anchors.
- Rendered charts map back to table anchors with `display: chart`.
- Rendered images map back to image anchors and image metadata.
- Rendered text objects map back to Markdown source blocks.
- Page-map source references validate against source objects.
- Chart page-map entries include table ID, view ID, bounding box, and rendered asset when present.
- MCD-Rendered validation recognizes layout and page-map files.

## Phase 10: WASM and TypeScript

Implement after Rust and Python APIs are stable:

- `crates/mcd-wasm`.
- `bindings/typescript`.
- Package name `@mcd/parser`.
- Browser and Node byte-input API:

```ts
import { openMcd } from "@mcd/parser";

const doc = await openMcd(bytes);
const validation = doc.validate();
const blocks = doc.blocks();
const markdown = doc.markdown({ expandTables: true });
```

Rules:

- WASM parser accepts bytes or `ArrayBuffer`.
- No direct filesystem dependency.
- No network access.
- No native PDF dependency.

Acceptance criteria:

- Vitest covers loading fixtures from bytes.
- TypeScript declarations are generated.
- WASM package validates the same conformance fixtures as Rust where practical.

## Error and diagnostic model

All validation should return structured diagnostics:

```json
{
  "level": "error",
  "code": "csv.header.mismatch",
  "message": "CSV header does not match table schema.",
  "source": "tables/revenue.csv:1",
  "related": ["tables/revenue.schema.json"]
}
```

Diagnostic namespaces:

```text
package.*
manifest.*
markdown.*
directive.*
table.*
csv.*
schema.*
view.*
chart.*
image.*
asset.*
layout.*
page_map.*
render.*
integrity.*
security.*
```

## Testing strategy

Required Rust checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Recommended test categories:

- Unit tests for path validation, manifest parsing, directive parsing, and type coercion.
- Integration tests for full document validation.
- Snapshot tests for JSON exports, diagnostics, and expanded Markdown.
- Security tests for malicious archives.
- Fixture tests for all conformance examples.
- Property tests for path normalization and CSV type coercion edge cases.
- Image validation tests for roles, alt text, captions, hashes, media types, and SVG safety.
- Chart validation tests for source tables, view references, encoding columns, type compatibility, and rendered-asset provenance.

Python checks:

```bash
pytest
ruff check .
mypy mcd
```

TypeScript checks, when implemented:

```bash
npm test
npm run typecheck
```

## Release readiness checklist

Before the first alpha release:

- Dependency versions are pinned.
- `cargo audit` has no unresolved high-severity findings.
- License metadata exists in `Cargo.toml`, `pyproject.toml`, and future `package.json`.
- `LICENSE` and `NOTICE` are present.
- Docs/spec license notice is present.
- Schemas, examples, and fixtures have CC0-1.0 notices.
- CLI validates all conformance fixtures.
- CLI validates MCD-Images, MCD-Charts, and MCD-Strict visual fixtures.
- Python package builds on Windows, macOS, and Linux.
- README has install, validate, extract, and Python usage examples.
- Known limitations are documented.

## Recommended implementation order

1. Add license files, workspace, `mcd-core`, and `mcd-cli`.
2. Implement safe package reading.
3. Implement manifest parsing and diagnostics.
4. Create minimal valid and invalid fixtures.
5. Implement Markdown block parsing and table directive extraction.
6. Implement table schema parsing and CSV type validation.
7. Implement chart view validation as table-backed views.
8. Implement image directive parsing, image metadata, and visual asset validation.
9. Implement document stream and JSON export.
10. Implement expanded Markdown export.
11. Complete CLI validate/extract/init/pack/unpack.
12. Add Python bindings.
13. Add JSON schemas and broaden conformance fixtures.
14. Add HTML renderer with table, image, and chart output.
15. Add PDF/page-map work.
16. Add WASM/TypeScript wrapper.

The parser and validator should be considered the product core. Rendering, PDF compatibility, signatures, and browser support should build on top only after MCD-Core behavior is stable.
