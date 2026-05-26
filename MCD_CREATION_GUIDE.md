# MCD Creation Guide for LLM Agents

This guide explains how to create a valid `.mcd` file from scratch. Use it when an agent needs to generate an MCD document without starting from an existing PDF.

The process has two main steps:

1. Create an unpacked package using the canonical MCD file layout and syntax.
2. Convert that unpacked package into a `.mcd` file with `mcd pack`.

The unpacked package is the source of truth. The `.mcd` file is the packaged ZIP-style artifact.

## Quick Recipe

```bash
mcd init work/report
# edit work/report/manifest.json
# edit work/report/content/main.md
# add tables, images, annotations, and layout files as needed
mcd pack work/report --output report.mcd
mcd validate report.mcd
```

When running from this repository without installing the CLI, replace `mcd` with:

```bash
cargo run -p mcd-cli --
```

For example:

```bash
cargo run -p mcd-cli -- pack work/report --output report.mcd
cargo run -p mcd-cli -- validate report.mcd
```

## Step 1: Create the Unpacked Package

### Minimal Directory Layout

Every package starts with this layout:

```text
report/
  mimetype
  manifest.json
  content/
    main.md
```

The root `mimetype` file should contain exactly this media type, plus an optional trailing newline:

```text
application/vnd.mcd+zip
```

The `mcd pack` command can create the mimetype entry automatically if the source directory does not contain one, but agents should usually write it explicitly.

### Recommended Full Layout

Use only the directories that the document needs:

```text
report/
  mimetype
  manifest.json
  content/
    main.md
  tables/
    revenue.csv
    revenue.schema.json
    revenue.view.json
    revenue.chart.view.json
  assets/
    process-diagram.svg
  images/
    process-diagram.image.json
  annotations/
    review-note.annotation.json
  provenance/
    provenance.json
  layout/
    styles.json
    page-map.json
```

### Package Path Rules

All paths inside the package must be safe package paths:

- Use forward slashes, even on Windows: `content/main.md`.
- Do not start paths with `/`.
- Do not use `.` or `..` path segments.
- Do not use backslashes or drive prefixes.
- Keep paths relative to the unpacked package root.

Valid paths:

```text
content/main.md
tables/revenue.csv
assets/process-diagram.svg
images/process-diagram.image.json
```

Invalid paths:

```text
/content/main.md
../outside.txt
content\main.md
C:\tmp\file.txt
```

### Identifier Rules

Use stable IDs for tables, images, annotations, views, and placements.

General IDs must match this shape:

```text
^[A-Za-z0-9][A-Za-z0-9_.-]*$
```

Column names must match this shape:

```text
^[A-Za-z_][A-Za-z0-9_.-]*$
```

Good IDs:

```text
revenue
quarterly-bar-chart
process-diagram
note-page-01
```

Avoid spaces, slashes, colons, and generated-looking random strings unless they are actually stable.

## Manifest

The root `manifest.json` declares the package entrypoint and all sidecar data files.

### Minimal Manifest

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md"
}
```

Required fields:

| Field | Required value |
| --- | --- |
| `format` | `MCD` |
| `version` | `0.1` |
| `profile` | Usually `MCD-Core` |
| `entrypoint` | Markdown source path, usually `content/main.md` |

Optional profile values are `MCD-Core`, `MCD-Rendered`, `MCD-Verified`, and `MCD-Signed`.

Optional conformance claims are `MCD-Core`, `MCD-Images`, `MCD-Charts`, and `MCD-Strict`.

### Manifest With Tables, Images, Annotations, and Layout

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "conformance": ["MCD-Core", "MCD-Images", "MCD-Charts", "MCD-Strict"],
  "entrypoint": "content/main.md",
  "title": "Quarterly Operating Report",
  "tables": [
    {
      "id": "revenue",
      "data": "tables/revenue.csv",
      "schema": "tables/revenue.schema.json",
      "views": {
        "default": "tables/revenue.view.json",
        "quarterly-bar-chart": "tables/revenue.chart.view.json"
      }
    }
  ],
  "images": [
    {
      "id": "process-diagram",
      "metadata": "images/process-diagram.image.json"
    }
  ],
  "annotations": [
    {
      "id": "review-note",
      "metadata": "annotations/review-note.annotation.json"
    }
  ],
  "externalData": [
    {
      "id": "raw-sensor-log",
      "uri": "https://example.com/datasets/raw-sensor-log.csv",
      "mediaType": "text/csv",
      "hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
      "sizeBytes": 1048576,
      "description": "Raw sensor log used to derive the summarized package tables.",
      "access": {
        "requiresNetwork": true,
        "requiresAuthentication": false,
        "notes": "Public HTTPS dataset."
      }
    }
  ],
  "provenance": "provenance/provenance.json",
  "layout": {
    "styles": "layout/styles.json",
    "pageMap": "layout/page-map.json"
  }
}
```

Use `externalData` for large or governed datasets that should not be stored inside the `.mcd` archive. The validator checks declaration shape only; it does not fetch external resources. Use absolute `http`, `https`, `s3`, `gs`, `file`, or `ipfs` URIs. Add a `sha256:` hash when deterministic retrieval matters.

Use `provenance` for one package-level sidecar that records source documents, actors, tools, generated assets, hashes, and timestamps. The sidecar should usually live at `provenance/provenance.json`.

Minimal provenance sidecar:

```json
{
  "sources": [
    {
      "id": "source-pdf",
      "path": "assets/source.pdf",
      "mediaType": "application/pdf",
      "hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
      "createdAt": "2026-05-26T10:00:00Z"
    }
  ],
  "tools": [
    {
      "id": "extractor",
      "name": "mcd-pdf-extract",
      "version": "0.1.0"
    }
  ],
  "actors": [
    {
      "id": "agent-1",
      "kind": "agent",
      "name": "Extraction agent"
    }
  ],
  "generatedAssets": [
    {
      "id": "main-md",
      "path": "content/main.md",
      "mediaType": "text/markdown",
      "createdAt": "2026-05-26T10:01:00Z",
      "sourceRefs": ["source-pdf"],
      "toolRefs": ["extractor"],
      "actorRefs": ["agent-1"]
    }
  ],
  "activities": [
    {
      "id": "extract-1",
      "kind": "extracted",
      "startedAt": "2026-05-26T10:00:00Z",
      "endedAt": "2026-05-26T10:01:00Z",
      "sourceRefs": ["source-pdf"],
      "toolRefs": ["extractor"],
      "actorRefs": ["agent-1"],
      "inputRefs": ["source:source-pdf"],
      "outputRefs": ["generatedAsset:main-md", "path:content/main.md"]
    }
  ]
}
```

Do not declare a table, image, annotation, asset, or layout path unless the corresponding file exists.

## Markdown Content

Markdown prose lives in the manifest entrypoint, usually `content/main.md`.

Canonical rules:

- Put all meaningful prose in Markdown.
- Use ordinary Markdown for headings, paragraphs, lists, links, code, and math text.
- Do not store meaningful table data as Markdown pipe tables. Use CSV plus schema files instead.
- Place tables, charts, and images with fenced directives.
- Use directive field syntax exactly as `key: value`.
- Unknown directive fields are rejected by strict parsing.

### Minimal Markdown

```markdown
# Minimal MCD

This is a minimal Markdown CSV Document.
```

### Table Directive

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

Accepted table directive fields:

| Field | Required | Meaning |
| --- | --- | --- |
| `table` | Yes | Table ID declared in `manifest.json` |
| `ref` | No | Stable placement ID |
| `view` | No | View ID from the table manifest entry |
| `display` | No | `table` or `chart`; defaults to `table` |
| `caption` | No | Placement caption |
| `numbering` | No | Numbering hint such as `auto` |
| `annotation` | No | Single annotation ID |
| `annotations` | No | Comma-separated annotation IDs |

### Chart Directive

Charts are table placements with `display: chart`. A chart directive must include a chart view.

```markdown
:::table
ref: revenue-chart
table: revenue
view: quarterly-bar-chart
display: chart
caption: Revenue by quarter
:::
```

### Image Directive

```markdown
:::image
ref: process-diagram-placement
asset: process-diagram
caption: Facility workflow showing intake, processing, quality control, and dispatch.
alt: Diagram of the facility workflow from intake to dispatch.
:::
```

Accepted image directive fields:

| Field | Required | Meaning |
| --- | --- | --- |
| `asset` or `image` | Yes | Image ID or direct asset reference |
| `ref` | No | Stable placement ID |
| `caption` | No | Placement caption |
| `alt` | No | Placement alt text |
| `annotation` | No | Single annotation ID |
| `annotations` | No | Comma-separated annotation IDs |

Prefer `asset: <image-id>` when the image is declared in `manifest.json` through `images`.

## Tables

Use a CSV file plus a JSON schema. Add one or more view files when the table is placed in Markdown or rendered as a chart.

### CSV

The CSV header must exactly match the schema column names and order.

`tables/revenue.csv`:

```csv
quarter,revenue_gbp
Q1,125000.00
Q2,141500.50
Q3,138250.25
Q4,167900.00
```

CSV typing rules:

- `integer` cells must parse as integers.
- `decimal` cells must parse as decimals.
- `boolean` cells must be `true` or `false` using any of `true`, `TRUE`, `True`, `false`, `FALSE`, or `False`.
- `date` cells must use `YYYY-MM-DD`.
- `datetime` cells must use RFC3339, such as `2026-05-19T10:30:00Z`, or local `YYYY-MM-DDTHH:MM:SS`.
- `time` cells must use `HH:MM` or `HH:MM:SS`.
- Empty cells are allowed only when the schema column has `"nullable": true`.

### Table Schema

`tables/revenue.schema.json`:

```json
{
  "id": "revenue",
  "primaryKey": ["quarter"],
  "columns": [
    {
      "name": "quarter",
      "type": "string",
      "label": "Quarter",
      "nullable": false
    },
    {
      "name": "revenue_gbp",
      "type": "decimal",
      "label": "Revenue",
      "nullable": false
    }
  ]
}
```

Use `primaryKey` when one or more columns uniquely identify rows. Primary-key columns must exist, must be non-nullable, and their combined values must be unique across the CSV.

Use `foreignKeys` when rows reference rows in another manifest-declared table:

```json
{
  "id": "orders",
  "primaryKey": ["order_id"],
  "foreignKeys": [
    {
      "columns": ["customer_id"],
      "references": {
        "table": "customers",
        "columns": ["customer_id"]
      }
    }
  ],
  "columns": [
    { "name": "order_id", "type": "string", "nullable": false },
    { "name": "customer_id", "type": "string", "nullable": false }
  ]
}
```

Foreign keys must reference the target table's `primaryKey`, use compatible column types, and resolve to existing target rows.

Allowed column types:

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

For `enum`, provide `enumValues` or `values`:

```json
{
  "name": "status",
  "type": "enum",
  "enumValues": ["open", "closed"]
}
```

### Table View

`tables/revenue.view.json`:

```json
{
  "id": "default",
  "table": "revenue",
  "display": "table",
  "columns": [
    {
      "name": "quarter",
      "label": "Quarter"
    },
    {
      "name": "revenue_gbp",
      "label": "Revenue",
      "format": "currency",
      "currency": "GBP"
    }
  ]
}
```

View columns must reference columns from the table schema.

### Chart View

`tables/revenue.chart.view.json`:

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
    }
  }
}
```

Allowed chart types:

```text
bar
line
area
scatter
```

Chart `x`, `y`, `series`, `grouping`, and `markLabels.column` values must reference existing schema columns. Chart axes must use compatible data types: use text-like columns for categories and numeric/date/time columns where the chart expects measured values.

## Images

Images use asset files plus image metadata.

### Image Metadata

`images/process-diagram.image.json`:

```json
{
  "id": "process-diagram",
  "asset": "assets/process-diagram.svg",
  "mediaType": "image/svg+xml",
  "role": "diagram",
  "caption": "Facility workflow showing intake, processing, quality control, and dispatch.",
  "alt": "Diagram of the facility workflow from intake to dispatch.",
  "intrinsicSize": {
    "width": 640,
    "height": 180,
    "unit": "px"
  }
}
```

Allowed media types:

```text
image/svg+xml
image/png
image/jpeg
image/webp
image/gif
```

Allowed roles:

```text
decorative
informative
diagram
photo
logo
rendered-table-prohibited
rendered-text-prohibited
```

Image rules:

- `informative`, `diagram`, `photo`, and `logo` images require non-empty `alt`.
- `informative` and `diagram` images require non-empty `caption`.
- SVG files must not contain scripts or external resource references.
- Under `MCD-Strict`, do not use an image as the only source of meaningful text, numbers, or table data. Put meaningful text in Markdown and meaningful tables in CSV.

If an image contains meaningful content, link it back to canonical Markdown or table sources:

```json
{
  "meaningfulContent": {
    "text": true,
    "markdownRefs": ["process-summary"]
  }
}
```

or:

```json
{
  "meaningfulContent": {
    "tableData": true,
    "tableRefs": ["revenue"]
  }
}
```

## Annotations

Annotations are optional metadata files declared in the manifest and referenced from Markdown directives or inline markers.

`annotations/review-note.annotation.json`:

```json
{
  "id": "review-note",
  "target": {
    "type": "path",
    "path": "content/main.md",
    "source": {
      "startLine": 3,
      "startColumn": 1,
      "endLine": 3,
      "endColumn": 80
    }
  },
  "kind": "comment",
  "status": "open",
  "body": "Confirm that the revenue figures match the source system.",
  "author": "agent",
  "created": "2026-05-19T00:00:00Z",
  "labels": ["review"]
}
```

Allowed annotation kinds:

```text
comment
flag
proposed_change
question
todo
```

Allowed annotation statuses:

```text
open
accepted
rejected
resolved
```

Allowed target types:

- `document`
- `block`
- `placement`
- `table`
- `image`
- `path`

For a simple document-level note:

```json
{
  "id": "review-note",
  "target": { "type": "document" },
  "kind": "comment",
  "status": "open",
  "body": "Review before publication."
}
```

The CLI can add a simple annotation to an already packed file:

```bash
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md --line 18 --id review-intro
```

## Layout Metadata

Layout is optional. Use it when the package needs page maps or style hints.

`layout/styles.json`:

```json
{
  "id": "default",
  "fonts": {
    "body": "Arial",
    "mono": "Consolas"
  },
  "colors": {
    "text": "#111827",
    "background": "#ffffff",
    "accent": "#2563eb"
  },
  "page": {
    "size": "A4",
    "margin": "24mm"
  },
  "body": {
    "fontSize": 11,
    "lineHeight": 1.45
  }
}
```

`layout/page-map.json`:

```json
{
  "pages": [
    {
      "number": 1,
      "label": "1",
      "sourceRefs": ["revenue-table", "revenue-chart"],
      "assets": ["assets/process-diagram.svg"]
    }
  ]
}
```

If the manifest declares layout files, those files must exist.

## Complete Small Table Package

Use this as a compact template for a document with one table and one chart.

```text
report/
  mimetype
  manifest.json
  content/
    main.md
  tables/
    revenue.csv
    revenue.schema.json
    revenue.view.json
    revenue.chart.view.json
```

`manifest.json`:

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "conformance": ["MCD-Core", "MCD-Charts"],
  "entrypoint": "content/main.md",
  "title": "Revenue Report",
  "tables": [
    {
      "id": "revenue",
      "data": "tables/revenue.csv",
      "schema": "tables/revenue.schema.json",
      "views": {
        "default": "tables/revenue.view.json",
        "quarterly-bar-chart": "tables/revenue.chart.view.json"
      }
    }
  ]
}
```

`content/main.md`:

```markdown
# Revenue Report

Quarterly revenue is tracked as canonical CSV-backed table data.

:::table
ref: revenue-table
table: revenue
view: default
display: table
caption: Revenue by quarter
numbering: auto
:::

The same source table can also be rendered as a chart.

:::table
ref: revenue-chart
table: revenue
view: quarterly-bar-chart
display: chart
caption: Revenue by quarter
:::
```

`tables/revenue.csv`:

```csv
quarter,revenue_gbp
Q1,125000.00
Q2,141500.50
Q3,138250.25
Q4,167900.00
```

`tables/revenue.schema.json`:

```json
{
  "id": "revenue",
  "columns": [
    {
      "name": "quarter",
      "type": "string",
      "label": "Quarter",
      "nullable": false
    },
    {
      "name": "revenue_gbp",
      "type": "decimal",
      "label": "Revenue",
      "nullable": false
    }
  ]
}
```

`tables/revenue.view.json`:

```json
{
  "id": "default",
  "table": "revenue",
  "display": "table",
  "columns": [
    {
      "name": "quarter",
      "label": "Quarter"
    },
    {
      "name": "revenue_gbp",
      "label": "Revenue",
      "format": "currency",
      "currency": "GBP"
    }
  ]
}
```

`tables/revenue.chart.view.json`:

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
    }
  }
}
```

## Step 2: Convert the Package Into a `.mcd` File

Pack the unpacked directory:

```bash
mcd pack report --output report.mcd
```

Validate the result:

```bash
mcd validate report.mcd
```

Inspect the result:

```bash
mcd inspect report.mcd
```

Render the result for a human check:

```bash
mcd render report.mcd --html --output report.html
```

Extract the canonical machine-readable projections:

```bash
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --json
mcd extract report.mcd --tables
mcd extract report.mcd --charts
mcd extract report.mcd --images
mcd extract report.mcd --annotations
```

## Agent Checklist

Before packing:

- `manifest.json` exists and uses `format: MCD`, `version: 0.1`, and a valid `entrypoint`.
- Every manifest path exists in the unpacked package.
- Every manifest ID is unique within its category.
- Every Markdown table directive references a declared table.
- Every chart directive has `display: chart` and references a chart view.
- Every image directive includes `asset` or `image`.
- Every CSV has a header row.
- Every CSV header exactly matches the schema column names and order.
- Every non-empty CSV cell matches its schema type.
- Empty CSV cells appear only in nullable columns.
- Every table view column exists in the table schema.
- Every chart encoding column exists in the table schema.
- Informative images have `alt`; diagram and informative images have `caption`.
- Package paths use forward slashes and do not escape the package root.

After packing:

- Run `mcd validate report.mcd`.
- Run `mcd render report.mcd --html --output report.html` for a visual sanity check when rendering matters.
- Run extraction commands for the content types the document claims to support.

## Common Failure Causes

| Symptom | Fix |
| --- | --- |
| `manifest.version.unsupported` | Use `"version": "0.1"`. |
| `manifest.entrypoint.invalid` | Use a relative forward-slash package path such as `content/main.md`. |
| `table.anchor.unresolved` | Declare the table in `manifest.json` or fix the Markdown `table:` ID. |
| `chart.view.required` | Add `view:` to chart directives. |
| `chart.view.not_chart` | Point the chart directive at a view with `"display": "chart"`. |
| `csv.header.mismatch` | Make CSV headers exactly match schema column names and order. |
| `csv.cell.empty.nonnullable` | Fill the cell or set `"nullable": true` on that schema column. |
| `view.column.unknown` | Fix the view column name or add the column to the schema and CSV. |
| `image.alt.missing` | Add non-empty `alt` for informative, diagram, photo, or logo images. |
| `security.svg.active_content` | Remove scripts and active content from SVG assets. |
| `security.svg.external_reference` | Remove external references from SVG assets. |

## Canonical Principle

Every meaningful object must come from one of these sources:

- Markdown prose in `content/main.md`.
- Typed CSV data plus table schemas in `tables/`.
- Declared metadata in JSON sidecar files.

Do not hide meaningful text, numbers, or tables only inside rendered images or exported HTML/PDF. Rendering is derived from the package source, not the other way around.
