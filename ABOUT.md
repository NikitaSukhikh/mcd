# MCD — Markdown CSV Document

**MCD** stands for **Markdown CSV Document**.

An `.mcd` file is a ZIP-like document package designed to look like a PDF for humans while remaining fully machine-readable for software, parsers, and AI agents.

MCD is built around three canonical layers:

1. **Markdown text** — all prose, headings, symbols, formulas, lists, references, and ordinary document text.
2. **Typed CSV tables** — all meaningful tables stored as external CSV data plus schemas and display views.
3. **Machine-readable layout** — all rendering rules, styles, page geometry, colors, dimensions, and source-to-page mappings.

The rendered page is not the source of truth. The source of truth is Markdown + CSV + schemas + layout metadata.

```text
MCD package
  → Markdown document flow
  → typed CSV tables
  → layout/style rules
  → deterministic PDF-like rendering
  → optional PDF/HTML export
```

MCD is not a PDF parser, not a richer PDF tag layer, and not a domain-specific taxonomy system. It is a document format where the content is already structured before rendering.

---

## 1. Project goal

The goal is to create a document format that satisfies both of these requirements:

```text
Human requirement:
  The document should look like a polished PDF.

Machine requirement:
  The document should be fully readable as Markdown text, typed tables, and layout metadata.
```

A human should be able to open an `.mcd` document and see pages, typography, headers, footers, tables, captions, footnotes, colors, and other familiar PDF-like presentation.

A machine should be able to open the same `.mcd` document and extract:

```text
- document order
- headings
- paragraphs
- lists
- formulas written as text
- exact table locations in the Markdown flow
- typed table data
- table schemas
- table display rules
- layout styles
- page positions
- source-to-render mappings
```

No OCR, no table guessing, no coordinate-based reconstruction, and no LLM heuristics should be required.

---

## 2. Core idea

PDF starts from a visual page model. Text and tables are placed on the page, and software later tries to reconstruct meaning.

MCD starts from a semantic source model. Text and tables are defined first, then pages are rendered from that source.

```text
PDF:
  page objects → attempted extraction → guessed text/tables

MCD:
  Markdown + CSV + schema → deterministic rendering → exact extraction
```

The main design rule is:

> Every visible meaningful object must originate from Markdown text, typed CSV table data, or declared layout metadata.

---

## 3. Non-goals

MCD intentionally avoids several goals that would make the first version too broad.

MCD is **not**:

```text
- a universal semantic ontology
- a replacement for every domain schema
- a legal/financial/medical taxonomy system
- an Inline XBRL replacement
- a DOCX clone
- a PDF-internal tagging system
- a scanned-document OCR format
- an executable document format
- a notebook format
- a JavaScript widget platform
- a database engine
```

MCD does not try to understand domain meaning directly. Contracts, financial reports, research papers, procurement documents, invoices, and lab reports can all be represented, but their meaning is expressed through Markdown text and typed tables.

The format only guarantees that machines can read the document structure, tables, and layout exactly. It does not guarantee that the author's claims are true.

---

## 4. Design principles

### 4.1 Markdown is the canonical prose layer

All meaningful prose lives in Markdown.

Markdown contains:

```text
- headings
- paragraphs
- lists
- block quotes
- code blocks
- links
- footnotes
- citations as text or Markdown-compatible references
- inline math as text
- block math as text
- special symbols as Unicode text
- table anchors
```

Markdown does **not** contain canonical table data. Tables are referenced from Markdown and stored separately.

### 4.2 Tables are external typed data

All meaningful tables live outside Markdown as CSV files plus schemas.

A Markdown document contains a table anchor such as:

```markdown
:::table
ref: quarterly-performance-table
table: quarterly-performance
view: default
caption: Quarterly performance metrics
:::
```

The table data lives in:

```text
tables/quarterly-performance.csv
tables/quarterly-performance.schema.json
tables/quarterly-performance.view.json
```

This avoids weak Markdown pipe tables and makes data extraction exact.

### 4.3 Layout is readable data

Layout is not hidden inside the PDF rendering. It is stored as machine-readable metadata.

Layout includes:

```text
- page size
- margins
- font declarations
- font sizes
- line heights
- colors
- spacing
- table borders
- column widths
- headers
- footers
- page numbers
- object positions
- page breaks
```

### 4.4 Rendering is derived

The PDF-like view is derived from canonical Markdown, tables, and layout rules.

```text
content/main.md
+ tables/*.csv
+ tables/*.schema.json
+ layout/styles.json
= rendered pages
```

PDF and HTML exports may be included in the package, but they are compatibility outputs, not the source of truth.

### 4.5 No duplicated LLM file

MCD does **not** require a stored `llm.md` file.

A native parser can generate an expanded Markdown view on demand:

```python
doc.to_markdown(expand_tables=True)
```

This avoids duplicated content and prevents drift between a generated LLM view and the canonical source.

### 4.6 Native parser first

MCD should be consumed through a native parser, similar in convenience to Python PDF parsers, but without the unreliability of PDF extraction.

A parser should expose:

```text
- Markdown content
- document blocks
- table anchors
- typed tables
- schemas
- table views
- layout rules
- page map
- validation results
```

---

## 5. File extension and MIME type

Recommended file extension:

```text
.mcd
```

Recommended MIME type:

```text
application/vnd.mcd+zip
```

The file is a ZIP-like package with a fixed internal structure.

The package should contain a root-level `mimetype` file with the exact content:

```text
application/vnd.mcd+zip
```

This allows tools to identify the package without guessing from the file extension alone.

---

## 6. Package structure

A typical `.mcd` package:

```text
annual-report.mcd
  mimetype
  manifest.json

  content/
    main.md

  tables/
    quarterly-performance.csv
    quarterly-performance.schema.json
    quarterly-performance.view.json

    employees.csv
    employees.schema.json
    employees.view.json

  layout/
    styles.json
    page-map.json

  render/
    report.pdf
    report.html

  integrity/
    checksums.json
    signatures.json
```

Only some files are mandatory.

### 6.1 Mandatory files

```text
mimetype
manifest.json
content/main.md
```

If the document contains tables, each table must have:

```text
tables/<table-id>.csv
tables/<table-id>.schema.json
```

### 6.2 Optional files

```text
tables/<table-id>.view.json
layout/styles.json
layout/page-map.json
render/report.pdf
render/report.html
integrity/checksums.json
integrity/signatures.json
```

### 6.3 Canonical source files

The canonical semantic source consists of:

```text
content/main.md
tables/*.csv
tables/*.schema.json
tables/*.view.json
layout/styles.json
```

### 6.4 Generated or derived files

The following files may be generated from the canonical source:

```text
layout/page-map.json
render/report.pdf
render/report.html
integrity/checksums.json
```

A validator may verify that generated files match the canonical source.

---

## 7. Manifest

The root `manifest.json` is the package index.

It declares:

```text
- format name
- format version
- conformance profile
- entrypoint Markdown file
- table IDs and paths
- layout paths
- render outputs
- integrity files
```

### 7.1 Minimal manifest

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md"
}
```

### 7.2 Manifest with tables

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md",
  "tables": [
    {
      "id": "quarterly-performance",
      "data": "tables/quarterly-performance.csv",
      "schema": "tables/quarterly-performance.schema.json",
      "views": {
        "default": "tables/quarterly-performance.view.json"
      }
    }
  ]
}
```

### 7.3 Full manifest example

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Rendered",
  "title": "Annual Report 2026",
  "entrypoint": "content/main.md",
  "encoding": "UTF-8",
  "tables": [
    {
      "id": "quarterly-performance",
      "data": "tables/quarterly-performance.csv",
      "schema": "tables/quarterly-performance.schema.json",
      "views": {
        "default": "tables/quarterly-performance.view.json"
      }
    },
    {
      "id": "employees",
      "data": "tables/employees.csv",
      "schema": "tables/employees.schema.json",
      "views": {
        "default": "tables/employees.view.json"
      }
    }
  ],
  "layout": {
    "styles": "layout/styles.json",
    "pageMap": "layout/page-map.json"
  },
  "renderings": [
    {
      "type": "pdf",
      "path": "render/report.pdf"
    },
    {
      "type": "html",
      "path": "render/report.html"
    }
  ],
  "integrity": {
    "checksums": "integrity/checksums.json",
    "signatures": "integrity/signatures.json"
  }
}
```

---

## 8. Markdown content layer

The Markdown file defines the document flow.

Example `content/main.md`:

```markdown
# Annual Report 2026

## Revenue summary

Revenue increased in every quarter.

:::table
ref: quarterly-performance-table
table: quarterly-performance
view: default
caption: Quarterly performance metrics
:::

The strongest quarter was Q4.

The growth formula used in this report is:

$$
Growth = \frac{Revenue_{current} - Revenue_{prior}}{Revenue_{prior}}
$$
```

The parser reads this as:

```text
heading
heading
paragraph
tableRef
paragraph
paragraph
mathBlock
```

The table appears exactly where the table anchor appears.

---

## 9. Markdown rules

MCD Markdown should be based on a strict Markdown dialect.

Recommended base:

```text
CommonMark-compatible Markdown
+ MCD table directive extension
+ optional math block convention
```

### 9.1 Supported Markdown constructs

MCD-Core should support:

```text
- headings
- paragraphs
- emphasis
- strong emphasis
- inline code
- fenced code blocks
- ordered lists
- unordered lists
- block quotes
- links
- footnotes, if the parser implementation supports them
- inline math written as text
- block math written as text
- table anchors
```

### 9.2 Math and symbols

Math is stored as text.

Inline math:

```markdown
The value of $x^2 + y^2$ is calculated below.
```

Block math:

```markdown
$$
Growth = \frac{Revenue_{current} - Revenue_{prior}}{Revenue_{prior}}
$$
```

Special symbols should be stored directly as Unicode when possible:

```markdown
Revenue increased by ≥ 10% in Q4.
```

If LaTeX-like symbols are used, they remain plain text and are still machine-readable.

### 9.3 Markdown pipe tables

MCD-Core should not use Markdown pipe tables as canonical semantic tables.

This is not recommended for canonical table data:

```markdown
| Quarter | Revenue |
|---|---:|
| Q1 | £120,000 |
| Q2 | £132,000 |
```

Reason: Markdown pipe tables do not reliably encode types, units, nullability, precision, display formatting, primary keys, or provenance.

In MCD-Core, meaningful tables should use table anchors and external typed CSV data.

A parser may support Markdown pipe tables as a compatibility feature, but they should be classified as one of the following:

```text
- plain Markdown text table
- authoring shorthand converted into external CSV/schema
- non-conforming semantic table in strict mode
```

Strict validation should fail if a document claims MCD-Core conformance while using Markdown pipe tables as meaningful tables without an external schema.

---

## 10. Table anchors in Markdown

Tables are placed inside Markdown using block-level table anchors.

### 10.1 Minimal table anchor

```markdown
:::table
table: quarterly-performance
:::
```

Meaning:

```text
Insert the table with ID `quarterly-performance` here.
```

### 10.2 Full table anchor

```markdown
:::table
ref: quarterly-performance-table
table: quarterly-performance
view: default
caption: Quarterly performance metrics
numbering: auto
:::
```

### 10.3 Table anchor fields

| Field | Required | Meaning |
|---|---:|---|
| `table` | Yes | ID of the underlying table data declared in `manifest.json` |
| `ref` | No, but recommended | Unique ID for this placement in the Markdown document |
| `view` | No | Display view to use for rendering |
| `caption` | No | Human-visible table caption |
| `numbering` | No | Table numbering rule, such as `auto` or `none` |

### 10.4 Data ID versus placement ID

MCD distinguishes between table data and table placement.

```text
table = data object ID
ref   = placement object ID
```

The same table may be inserted multiple times using different placements or views.

```markdown
:::table
ref: revenue-full-table
table: revenue
view: full
caption: Full revenue table
:::

:::table
ref: revenue-summary-table
table: revenue
view: summary
caption: Revenue summary
:::
```

The parser should treat these as two table references pointing to the same underlying data.

---

## 11. Table data layer

Tables are stored as CSV.

Example `tables/quarterly-performance.csv`:

```csv
quarter,revenue_gbp,margin_percent
Q1,120000,18.4
Q2,132000,19.1
Q3,141000,20.0
Q4,169000,22.7
```

The CSV file contains raw data values, not display strings.

Bad canonical data:

```csv
quarter,revenue
Q1,"£120,000"
Q2,"£132,000"
```

Better canonical data:

```csv
quarter,revenue_gbp
Q1,120000
Q2,132000
```

The display formatting belongs in the table view.

---

## 12. Table schema

Every table must have a schema.

Example `tables/quarterly-performance.schema.json`:

```json
{
  "id": "quarterly-performance",
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
      "unit": "GBP",
      "label": "Revenue",
      "nullable": false
    },
    {
      "name": "margin_percent",
      "type": "decimal",
      "unit": "percent",
      "label": "Margin",
      "nullable": false
    }
  ]
}
```

### 12.1 Required schema fields

At minimum, a table schema should define:

```text
- table ID
- columns
- column names
- column types
```

### 12.2 Recommended schema fields

A schema may also define:

```text
- labels
- units
- nullability
- primary key
- allowed values
- minimum values
- maximum values
- date formats
- decimal precision
- descriptions
```

### 12.3 Supported primitive types

MCD-Core should support these primitive column types:

| Type | Meaning |
|---|---|
| `string` | Unicode text |
| `integer` | Whole number |
| `decimal` | Decimal number |
| `boolean` | `true` or `false` |
| `date` | ISO date, such as `2026-05-17` |
| `datetime` | ISO timestamp |
| `time` | Time value |
| `enum` | One value from a declared set |

### 12.4 CSV value rules

Recommended rules:

```text
- CSV must be UTF-8.
- Header row is required.
- Column names must match schema column names.
- Decimal values should use `.` as the decimal separator.
- Currency symbols should not appear in raw data values.
- Percent signs should not appear in raw data values.
- Display formatting belongs in the view file.
- Empty values are allowed only if the schema column is nullable.
```

---

## 13. Table view

A table view controls how the table is displayed to humans.

Example `tables/quarterly-performance.view.json`:

```json
{
  "table": "quarterly-performance",
  "columns": [
    {
      "source": "quarter",
      "label": "Quarter",
      "align": "left"
    },
    {
      "source": "revenue_gbp",
      "label": "Revenue",
      "format": "currency",
      "currency": "GBP",
      "align": "right"
    },
    {
      "source": "margin_percent",
      "label": "Margin",
      "format": "percent",
      "align": "right"
    }
  ]
}
```

The table view may define:

```text
- visible columns
- column order
- display labels
- number formatting
- currency formatting
- percent formatting
- date formatting
- alignment
- preferred widths
- caption style
```

MCD-Core should keep table views declarative. Views should not contain arbitrary executable code.

### 13.1 Derived values

For v1, derived values should be avoided or declared explicitly in the table data.

Preferred:

```csv
quarter,revenue_gbp,prior_revenue_gbp,growth_percent
Q4,169000,141000,19.86
```

Avoid hidden view logic such as:

```text
Display growth = (revenue - prior_revenue) / prior_revenue
```

If derived columns are introduced later, they should use a restricted declarative formula system, not arbitrary scripts.

---

## 14. Layout layer

The layout layer describes how the document is rendered.

Example `layout/styles.json`:

```json
{
  "page": {
    "size": "A4",
    "marginTop": "24mm",
    "marginBottom": "24mm",
    "marginLeft": "22mm",
    "marginRight": "22mm"
  },
  "styles": {
    "heading1": {
      "fontFamily": "Serif",
      "fontSize": "22pt",
      "fontWeight": "bold",
      "marginAfter": "12pt"
    },
    "heading2": {
      "fontFamily": "Serif",
      "fontSize": "16pt",
      "fontWeight": "bold",
      "marginBefore": "18pt",
      "marginAfter": "8pt"
    },
    "paragraph": {
      "fontFamily": "Serif",
      "fontSize": "11pt",
      "lineHeight": 1.35,
      "marginAfter": "8pt"
    },
    "table": {
      "fontFamily": "Sans",
      "fontSize": "10pt",
      "borderCollapse": true,
      "cellPadding": "4pt"
    }
  }
}
```

The layout file is not supposed to define document meaning. It defines appearance.

Layout may describe:

```text
- page size
- margins
- columns
- headers
- footers
- text styles
- table styles
- colors
- spacing
- line height
- borders
- caption style
- page break behavior
```

---

## 15. Page map

The page map links rendered visual objects back to canonical source objects.

Example `layout/page-map.json`:

```json
{
  "renderer": {
    "name": "mcd-renderer",
    "version": "0.1.0"
  },
  "pages": [
    {
      "number": 1,
      "size": {
        "width": 595.28,
        "height": 841.89,
        "unit": "pt"
      },
      "objects": [
        {
          "id": "page1-heading1-001",
          "type": "heading",
          "source": "content/main.md:1",
          "sourceId": "md-h-001",
          "bbox": [72, 88, 460, 116],
          "style": "heading1"
        },
        {
          "id": "page1-paragraph-001",
          "type": "paragraph",
          "source": "content/main.md:5",
          "sourceId": "md-p-001",
          "bbox": [72, 136, 510, 168],
          "style": "paragraph"
        },
        {
          "id": "page1-table-001",
          "type": "table",
          "source": "content/main.md:7-12",
          "sourceId": "quarterly-performance-table",
          "table": "quarterly-performance",
          "bbox": [72, 196, 510, 310],
          "style": "table"
        }
      ]
    }
  ]
}
```

### 15.1 What the page map enables

The page map lets a machine answer questions such as:

```text
- What page is this paragraph on?
- Where is this table rendered?
- Which Markdown lines produced this visual object?
- Which table data produced this rendered table?
- Which style was applied to this object?
- What object appears before or after it on the page?
```

### 15.2 Source-to-render traceability

MCD should support bidirectional traceability:

```text
Markdown source → rendered page object
rendered page object → Markdown/table source
```

This is one of the main differences from PDF.

---

## 16. Rendering model

MCD rendering should be deterministic.

```text
same MCD source
+ same renderer version
+ same assets/fonts
= same page output
= same page map
```

The renderer takes:

```text
content/main.md
tables/*.csv
tables/*.schema.json
tables/*.view.json
layout/styles.json
```

and produces:

```text
layout/page-map.json
render/report.pdf
render/report.html
```

### 16.1 Rendering outputs

Recommended outputs:

```text
- PDF for compatibility
- HTML for browser display
- page-map JSON for machine layout grounding
```

### 16.2 Rendering constraints

Rendering should avoid:

```text
- arbitrary JavaScript
- remote resource loading by default
- macros
- executable plugins
- hidden layout mutation
- viewer-dependent logic
```

### 16.3 Rendering is not the source of truth

Even if `render/report.pdf` is included, the PDF is not canonical.

Canonical:

```text
Markdown + CSV + schema + layout
```

Derived:

```text
PDF + HTML + page-map
```

---

## 17. Parser model

The parser is the central developer-facing interface.

It should behave more like a structured-document parser than a PDF extractor.

```text
PDF parser:
  infer text, tables, and order from visual objects

MCD parser:
  read declared text, declared tables, and declared order
```

### 17.1 Parser responsibilities

The parser should:

```text
1. Open the `.mcd` package.
2. Read `mimetype`.
3. Read and validate `manifest.json`.
4. Load the Markdown entrypoint.
5. Parse Markdown into a document stream.
6. Detect table anchors.
7. Resolve table anchors through the manifest.
8. Load CSV data.
9. Load table schemas.
10. Type-check CSV values.
11. Load table views.
12. Load layout styles.
13. Load page map, if present.
14. Expose document blocks in canonical order.
15. Validate source/render consistency.
```

### 17.2 Parser output model

A parser should expose a canonical document stream.

Example:

```json
{
  "format": "MCD",
  "version": "0.1",
  "blocks": [
    {
      "type": "heading",
      "level": 1,
      "text": "Annual Report 2026",
      "source": "content/main.md:1",
      "sourceId": "md-h-001"
    },
    {
      "type": "paragraph",
      "text": "Revenue increased in every quarter.",
      "source": "content/main.md:5",
      "sourceId": "md-p-001"
    },
    {
      "type": "tableRef",
      "ref": "quarterly-performance-table",
      "table": "quarterly-performance",
      "view": "default",
      "caption": "Quarterly performance metrics",
      "source": "content/main.md:7-12"
    }
  ]
}
```

### 17.3 Expanded Markdown view

The parser may generate an expanded Markdown view on demand.

```python
doc.to_markdown(expand_tables=True)
```

Output:

```markdown
# Annual Report 2026

Revenue increased in every quarter.

[TABLE id="quarterly-performance" ref="quarterly-performance-table"]
| Quarter | Revenue | Margin |
|---|---:|---:|
| Q1 | £120,000 | 18.4% |
| Q2 | £132,000 | 19.1% |
| Q3 | £141,000 | 20.0% |
| Q4 | £169,000 | 22.7% |
[/TABLE]
```

This view is generated from canonical Markdown and CSV data. It is not stored as a separate source file.

---

## 18. Python API

The first public API should be Python because AI agents, data tools, notebooks, and document automation workflows use Python heavily.

Example:

```python
import mcd

doc = mcd.open("annual-report.mcd")

doc.validate()

print(doc.title)
print(doc.markdown())
print(doc.markdown(expand_tables=True))

for block in doc.blocks():
    print(block.type, block.source)

revenue = doc.table("quarterly-performance")
print(revenue.schema)
print(revenue.rows())

layout = doc.layout_for("quarterly-performance-table")
print(layout.page)
print(layout.bbox)
```

### 18.1 DataFrame support

The Python package should optionally expose tables as pandas DataFrames.

```python
df = doc.table("quarterly-performance").to_pandas()
```

This should be optional, not required by the core parser.

### 18.2 Agent context

A parser should expose a structured context suitable for AI agents.

```python
context = doc.to_agent_context(
    include_tables=True,
    include_layout=False
)
```

With layout:

```python
context = doc.to_agent_context(
    include_tables=True,
    include_layout=True
)
```

---

## 19. CLI interface

A command-line tool should be part of the reference implementation.

Recommended commands:

```bash
mcd validate report.mcd
mcd inspect report.mcd
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --json
mcd extract report.mcd --tables
mcd extract report.mcd --layout
mcd render report.mcd --pdf
mcd render report.mcd --html
mcd init new-document.mcd
```

### 19.1 Validation command

```bash
mcd validate annual-report.mcd
```

Expected output:

```text
MCD validation passed.
Profile: MCD-Core
Tables: 2
Table anchors: 2
Schemas: valid
CSV data: valid
Layout: valid
Page map: present
```

### 19.2 Extraction command

```bash
mcd extract annual-report.mcd --json
```

Should return the canonical document stream.

```bash
mcd extract annual-report.mcd --tables
```

Should return table metadata and data paths or table data itself depending on flags.

---

## 20. Recommended technology stack

### 20.1 Core parser and validator

Recommended core language:

```text
Rust
```

Reasons:

```text
- native performance
- memory safety
- deterministic parsing
- good ZIP/CSV/JSON ecosystem
- good CLI support
- good Python bindings
- WebAssembly path for browser/Node support
```

### 20.2 Python bindings

Recommended tooling:

```text
PyO3 + maturin
```

This allows the Rust parser to be exposed as a normal Python package.

### 20.3 Web and TypeScript support

Recommended path:

```text
Rust core → WebAssembly → TypeScript wrapper
```

This allows the same parser logic to work in:

```text
- browsers
- Node.js
- web viewers
- document portals
- AI web tools
```

### 20.4 Suggested repository layout

```text
mcd/
  README.md
  ABOUT.md
  SPEC.md

  crates/
    mcd-core/
    mcd-cli/
    mcd-render/

  bindings/
    python/
    wasm/
    typescript/

  examples/
    annual-report/
      annual-report.mcd
      unpacked/

  schemas/
    manifest.schema.json
    table.schema.json
    table-view.schema.json
    styles.schema.json
    page-map.schema.json

  tests/
    fixtures/
    conformance/
```

### 20.5 Suggested Rust crates and responsibilities

The exact dependencies may evolve, but the architecture should roughly map as follows:

```text
ZIP/package handling:
  open `.mcd`, prevent path traversal, read files

Markdown parsing:
  parse CommonMark-compatible Markdown and MCD table directives

JSON handling:
  parse manifest, schemas, views, layout, page maps

CSV handling:
  read table data, validate headers, coerce typed values

Validation:
  enforce MCD profile rules

Export:
  produce JSON document stream and expanded Markdown
```

---

## 21. Validation rules

Validation is what makes MCD trustworthy.

### 21.1 Core validation

A valid MCD-Core document must satisfy:

```text
- `mimetype` exists and matches `application/vnd.mcd+zip`.
- `manifest.json` exists and is valid JSON.
- Manifest `format` is `MCD`.
- Manifest `version` is supported.
- Entrypoint Markdown file exists.
- Markdown is valid for the supported dialect.
- Every table anchor has a `table` field.
- Every table anchor resolves to a declared table ID.
- Every table ID is unique.
- Every placement `ref` is unique when provided.
- Every declared table data file exists.
- Every declared table schema file exists.
- CSV headers match schema column names.
- CSV values conform to declared column types.
- Non-nullable columns do not contain empty values.
- Table views reference existing schema columns.
- Layout files are valid if present.
- Page-map files are valid if present.
```

### 21.2 Rendering validation

For MCD-Rendered or MCD-Verified profiles:

```text
- Every rendered text object maps back to Markdown.
- Every rendered table maps back to a table anchor.
- Every rendered table cell maps back to source table data.
- Rendered table values match source values after formatting.
- Every page-map source reference points to a valid source object.
- No visible semantic object exists only in the render layer.
```

### 21.3 Strict table validation

In strict mode:

```text
- Markdown pipe tables cannot be used as semantic tables.
- Tables must be external CSV + schema.
- Raw table values cannot contain display-only symbols such as currency signs unless the schema type is string.
- Percent values should be stored as numbers, not strings containing `%`.
- Dates must use declared formats, preferably ISO formats.
```

### 21.4 Integrity validation

If checksums are present:

```text
- All declared files must match their checksum.
- Manifest checksum references must be valid.
- Rendered output hashes must match the rendered files.
- Page-map hash must match the page-map file.
```

---

## 22. Conformance profiles

MCD should support multiple conformance profiles.

### 22.1 MCD-Lite

Minimal profile.

```text
- manifest
- Markdown entrypoint
- optional tables
- optional schemas
```

Useful for prototypes and simple documents.

### 22.2 MCD-Core

Main profile.

```text
- manifest
- Markdown entrypoint
- strict table anchors
- typed CSV tables
- table schemas
- optional table views
- no meaningful untyped tables
```

This should be the default target.

### 22.3 MCD-Rendered

Adds rendering metadata.

```text
- all MCD-Core rules
- layout styles
- page map
- optional PDF/HTML rendering
```

### 22.4 MCD-Verified

Adds render/source consistency.

```text
- all MCD-Rendered rules
- rendered text maps to Markdown
- rendered tables map to table anchors
- rendered table cells match source values after formatting
- no semantic visual object is render-only
```

### 22.5 MCD-Signed

Adds cryptographic integrity.

```text
- all MCD-Verified rules
- checksums
- digital signatures
- immutable render/source hash chain
```

---

## 23. Security model

MCD files should be safe to inspect and parse.

### 23.1 No executable content by default

MCD should not contain:

```text
- macros
- arbitrary JavaScript
- executable plugins
- shell commands
- active scripts
- network calls required for rendering
```

### 23.2 Safe ZIP handling

Parsers must defend against archive attacks.

Rules:

```text
- reject absolute paths
- reject `..` path traversal
- reject duplicate file names after normalization
- impose maximum file size limits
- impose maximum decompressed package size limits
- validate declared paths against manifest
```

### 23.3 Remote resources

Remote resources should be disabled by default.

If a future profile allows remote resources, they must be explicit and should not be required for core parsing.

### 23.4 Fonts and assets

Fonts and assets may be declared for rendering, but they should be:

```text
- local to the package
- listed in the manifest or layout
- hash-checked if integrity metadata is present
```

Meaningful text should not be embedded only inside an image asset.

---

## 24. AI agent consumption

AI agents should consume MCD through the native parser.

The agent should not need to manually inspect a ZIP archive.

Recommended flow:

```text
User provides report.mcd
→ agent runtime calls MCD parser
→ parser returns document stream + tables
→ LLM reads Markdown-like text and typed tables
→ optional layout grounding is loaded from page map
```

### 24.1 Basic agent read

```python
doc = mcd.open("report.mcd")
context = doc.to_agent_context(include_tables=True)
```

The agent receives:

```text
- document blocks in order
- table references at exact Markdown positions
- typed table data
- schemas and units
```

### 24.2 Layout-aware agent read

```python
context = doc.to_agent_context(
    include_tables=True,
    include_layout=True
)
```

The agent receives:

```text
- content
- tables
- page numbers
- bounding boxes
- style references
- page-map coordinates
```

### 24.3 Why no `llm.md`

A stored `llm.md` file duplicates canonical data.

Duplication creates risk:

```text
content.md changes but llm.md does not
CSV changes but llm.md does not
schema changes but llm.md does not
```

MCD avoids this by generating AI-readable output on demand from canonical files.

---

## 25. Example: complete small MCD document

### 25.1 Package tree

```text
example.mcd
  mimetype
  manifest.json
  content/
    main.md
  tables/
    revenue.csv
    revenue.schema.json
    revenue.view.json
  layout/
    styles.json
```

### 25.2 `mimetype`

```text
application/vnd.mcd+zip
```

### 25.3 `manifest.json`

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
    "styles": "layout/styles.json"
  }
}
```

### 25.4 `content/main.md`

```markdown
# Revenue summary

Revenue increased in every quarter.

:::table
ref: revenue-table
table: revenue
view: default
caption: Revenue by quarter
:::

The strongest quarter was Q4.
```

### 25.5 `tables/revenue.csv`

```csv
quarter,revenue_gbp,margin_percent
Q1,120000,18.4
Q2,132000,19.1
Q3,141000,20.0
Q4,169000,22.7
```

### 25.6 `tables/revenue.schema.json`

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
      "unit": "GBP",
      "label": "Revenue",
      "nullable": false
    },
    {
      "name": "margin_percent",
      "type": "decimal",
      "unit": "percent",
      "label": "Margin",
      "nullable": false
    }
  ]
}
```

### 25.7 `tables/revenue.view.json`

```json
{
  "table": "revenue",
  "columns": [
    {
      "source": "quarter",
      "label": "Quarter",
      "align": "left"
    },
    {
      "source": "revenue_gbp",
      "label": "Revenue",
      "format": "currency",
      "currency": "GBP",
      "align": "right"
    },
    {
      "source": "margin_percent",
      "label": "Margin",
      "format": "percent",
      "align": "right"
    }
  ]
}
```

### 25.8 Parser-expanded Markdown

Generated by:

```bash
mcd extract example.mcd --markdown --expand-tables
```

Output:

```markdown
# Revenue summary

Revenue increased in every quarter.

[TABLE id="revenue" ref="revenue-table" caption="Revenue by quarter"]
| Quarter | Revenue | Margin |
|---|---:|---:|
| Q1 | £120,000 | 18.4% |
| Q2 | £132,000 | 19.1% |
| Q3 | £141,000 | 20.0% |
| Q4 | £169,000 | 22.7% |
[/TABLE]

The strongest quarter was Q4.
```

---

## 26. Format grammar sketch

This is not a full formal grammar. It sketches the core package relationships.

```text
MCDPackage :=
  mimetype
  manifest.json
  MarkdownEntrypoint
  TableFiles*
  LayoutFiles?
  RenderFiles?
  IntegrityFiles?

MarkdownEntrypoint :=
  CommonMarkDocument + MCDTableDirectives

MCDTableDirective :=
  ":::table" newline
  TableDirectiveFields
  ":::"

TableDirectiveFields :=
  table: TableID
  ref: PlacementID?
  view: ViewID?
  caption: Text?
  numbering: NumberingRule?

TableObject :=
  CSVData + TableSchema + TableView?

TableSchema :=
  TableID + ColumnDefinition+

ColumnDefinition :=
  name + type + label? + unit? + nullable? + constraints?

Layout :=
  Styles + PageMap?
```

---

## 27. Naming conventions

Recommended naming rules:

```text
- Table IDs should use lowercase kebab-case.
- Placement refs should use lowercase kebab-case.
- File names should match table IDs where practical.
- Schema files should end in `.schema.json`.
- View files should end in `.view.json`.
```

Example:

```text
table ID: quarterly-performance
placement ref: quarterly-performance-table
data file: tables/quarterly-performance.csv
schema file: tables/quarterly-performance.schema.json
view file: tables/quarterly-performance.view.json
```

---

## 28. Handling images and figures

MCD's strict semantic model is text + tables + layout.

Images may exist for rendering, decoration, logos, or visual presentation, but meaningful information should not exist only as an image.

Recommended rule:

```text
If a visual element contains meaningful text or tabular data, that meaning must also exist in Markdown or CSV.
```

Examples:

```text
Company logo:
  allowed as visual asset

Decorative background:
  allowed as visual asset

Chart showing revenue:
  should be backed by a CSV table

Scanned page of a contract:
  not valid as full machine-readable MCD content unless transcribed into Markdown

Image containing a data table:
  not valid as full machine-readable MCD content unless the table exists as CSV + schema
```

For v1, charts can be represented as rendered views of tables, not as separate semantic objects.

---

## 29. Relationship to PDF

PDF remains useful as an export format.

MCD should be able to include:

```text
render/report.pdf
```

This lets humans open the file through ordinary PDF-compatible workflows when needed.

However:

```text
PDF is not canonical.
PDF extraction is not required.
PDF is a derived rendering.
```

A PDF export should be considered equivalent to a screenshot of the canonical document, except that it may contain selectable text and tagged structure if the renderer supports it.

---

## 30. Relationship to Markdown

MCD uses Markdown as the prose layer, but plain Markdown alone is not enough.

Markdown is good for:

```text
- human-readable writing
- headings
- paragraphs
- lists
- formulas as text
- simple authoring
```

Markdown is weak for:

```text
- typed tables
- units
- exact numeric values
- schema validation
- page layout
- source-to-render mapping
```

MCD keeps Markdown for prose and adds strict external table and layout layers.

---

## 31. Relationship to CSV

CSV is used because it is simple, inspectable, and widely supported.

However, raw CSV alone is not enough.

MCD requires schemas because CSV values are otherwise just strings.

```text
CSV:
  values

Schema:
  types, units, labels, constraints

View:
  human display formatting
```

This separation is central to the format.

---

## 32. Relationship to AI agents

MCD is designed to be easy for AI agents because it avoids ambiguous extraction.

An agent can receive:

```text
- Markdown text in document order
- typed tables at exact Markdown positions
- schemas with units and labels
- optional layout/page information
```

The agent does not need to infer whether a visual group of words is a table. The table is declared.

The agent does not need to infer whether `£120,000` is a string or a number. The raw value is `120000`, and the schema says `unit: GBP`.

The agent does not need to infer where the table belongs. The table anchor gives the exact Markdown position.

---

## 33. Build order

Recommended implementation sequence:

```text
1. Define manifest schema.
2. Define table schema format.
3. Define table view format.
4. Define table directive syntax.
5. Build Rust package reader.
6. Build Markdown parser integration.
7. Build table anchor resolver.
8. Build CSV + schema validator.
9. Build canonical document stream output.
10. Build CLI validator.
11. Build Python bindings.
12. Build expanded Markdown export.
13. Build layout schema.
14. Build page-map schema.
15. Build simple renderer.
16. Add PDF/HTML export.
17. Add render/source validation.
18. Add checksums and signatures.
19. Add WASM/TypeScript bindings.
```

The parser and validator should come before the renderer.

---

## 34. Minimal viable product

The first useful version should support:

```text
- `.mcd` ZIP package
- `mimetype`
- `manifest.json`
- `content/main.md`
- table anchors
- CSV tables
- JSON table schemas
- JSON table views
- parser-generated expanded Markdown
- parser-generated JSON document stream
- CLI validation
- Python API
```

Rendering can start simple:

```text
- HTML output first
- PDF output second
- page-map support after stable layout rules
```

---

## 35. Reference implementation modules

Recommended Rust core modules:

```text
mcd_core::package
  open package, read files, enforce path safety

mcd_core::manifest
  parse and validate manifest

mcd_core::markdown
  parse Markdown and identify table directives

mcd_core::tables
  load CSV, schema, view, type-check rows

mcd_core::document
  build canonical document stream

mcd_core::layout
  load styles and page map

mcd_core::validate
  run conformance validation

mcd_core::export
  export JSON and expanded Markdown
```

Recommended Python package modules:

```text
mcd.open
mcd.Document
mcd.Table
mcd.Schema
mcd.Layout
mcd.ValidationResult
```

---

## 36. Open questions

Several decisions should be finalized during early specification work.

```text
- Exact Markdown dialect.
- Exact table directive syntax.
- Whether table directive fields use YAML syntax or stricter key-value syntax.
- Whether schemas follow an existing standard or a custom minimal schema.
- Whether table views should allow sorting or filtering in v1.
- How strict math handling should be.
- How page-map granularity should work for table cells.
- Whether PDF export is required for MCD-Rendered or optional.
- Whether HTML rendering should be the first renderer.
- How signatures should be represented.
```

Recommended early choices:

```text
- Use a CommonMark-compatible Markdown parser.
- Use simple block directives for table anchors.
- Use custom minimal JSON schemas for tables first.
- Avoid transformations in table views for v1.
- Keep rendering deterministic and non-executable.
- Make PDF export optional in MCD-Core.
```

---

## 37. Summary

MCD is a strict, inspectable, parser-friendly document format.

Its core statement is:

```text
A document is Markdown plus typed CSV tables plus machine-readable layout.
```

Its rendering statement is:

```text
A PDF-like page view is derived from the canonical source, not used as the source.
```

Its AI statement is:

```text
Agents read the native parser output: Markdown blocks, typed tables, schemas, and optional layout maps.
```

Its validation statement is:

```text
Every meaningful visible object must trace back to Markdown text or typed table data.
```

The `.mcd` format should therefore provide:

```text
- high human readability
- exact machine readability
- typed table extraction
- deterministic rendering
- source-to-page traceability
- Python-friendly parsing
- optional PDF compatibility
```

The result is not “a better PDF.”

It is a structured document package that can render like a PDF while remaining readable like Markdown and extractable like a database.
