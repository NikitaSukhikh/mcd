# MCD MCP Guide for AI Agents

This guide describes how an AI agent should use the MCD MCP tools to inspect,
search, query, and update `.mcd` packages.

MCD packages contain Markdown prose, typed CSV-backed tables, schema metadata,
relationships, annotations, provenance, and renderable assets. Prefer MCP tools
over ad hoc ZIP inspection because the tools preserve package semantics and
return stable structured data.

## Server Setup

Use the Rust MCP server when possible. It has the complete tool set.

```bash
cargo install mcd-mcp --version 0.1.0-alpha.2
mcd-mcp --transport stdio
```

From a repository checkout:

```bash
cargo run -p mcd-mcp -- --transport stdio
```

Python environments can use the convenience server:

```bash
pip install "mcdee[mcp]"
mcd-python-mcp
```

The Python server exposes the main read and conversion tools. The Rust server
also exposes package mutation and rendering tools such as `mcd_pack`,
`mcd_unpack`, `mcd_init`, `mcd_render`, and `mcd_add_annotation`.

Tool names in this guide follow the Rust MCP server. The Python convenience
server exposes `mcd_image` instead of `mcd_images` and does not expose every
package creation or rendering tool.

## Recommended Agent Workflow

1. Validate the package with `mcd_validate`.
2. Inspect document structure with `mcd_agent_context`.
3. Use `mcd_search` to find relevant Markdown passages, schema fields,
   annotations, or provenance records.
4. Use `mcd_query` or `mcd_queries` for exact table-row analysis.
5. Use object-specific tools such as `mcd_table`, `mcd_chart`, `mcd_images`,
   `mcd_annotations`, `mcd_relationships`, `mcd_external_data`, or
   `mcd_provenance` when exact metadata is needed.
6. Answer with the package paths, table names, column names, filters, and source
   lines used.

Do not guess table names, column names, units, or join keys. Discover them with
`mcd_agent_context`, `mcd_search`, `mcd_schemas`, or SQL metadata tables first.

## Tool Selection

| Task | Use |
| --- | --- |
| Check package validity | `mcd_validate` |
| Get package overview | `mcd_inspect` or `mcd_agent_context` |
| Read document prose | `mcd_markdown` |
| Find relevant content | `mcd_search` |
| Filter, join, aggregate, or sort table rows | `mcd_query` |
| Run several SQL queries against one package | `mcd_queries` |
| Inspect one table and optional rows | `mcd_table` |
| Inspect schemas, keys, units, and relationships | `mcd_schemas` |
| Inspect chart definitions and source rows | `mcd_chart` |
| Inspect images | `mcd_images` |
| Inspect annotations | `mcd_annotations` |
| Inspect declared relationships | `mcd_relationships` |
| Inspect external references | `mcd_external_data` |
| Inspect provenance | `mcd_provenance` |
| Render package output | `mcd_render` |
| Convert PDF to MCD | `mcd_convert_pdf` |
| Pack or unpack packages | `mcd_pack`, `mcd_unpack` |
| Create a minimal package directory | `mcd_init` |
| Add a plain-text annotation | `mcd_add_annotation` |

## Search Guidance

Use `mcd_search` for retrieval across package-owned content and metadata.

Example call:

```json
{
  "path": "report.mcd",
  "query": "thermal_limit_deg_c coolant V50D",
  "limit": 5
}
```

Optional filters:

```json
{
  "path": "report.mcd",
  "query": "variant_id",
  "kind": "schema",
  "limit": 5
}
```

Supported `kind` values are `markdown`, `schema`, `manifest`, `annotation`,
and `provenance`. Use `page` to filter by an internal package path such as
`content/main.md`.

Search indexes Markdown blocks, table schema and column metadata, manifest
metadata, annotations, and provenance text. It intentionally does not index CSV
table rows. Use SQL for row-level questions.

Search hits include:

```json
{
  "path": "content/main.md",
  "kind": "markdown",
  "heading": "Powertrain calibration specifications",
  "line_start": 48,
  "line_end": 48,
  "score": 12.4,
  "text": "..."
}
```

Use the hit `path`, `heading`, and line fields when citing sources in an answer.

## SQL Guidance

Use `mcd_query` for exact table questions. Queries must be read-only `SELECT`
or `WITH` statements.

Start with metadata discovery:

```sql
select table_id, data_path, schema_path
from mcd_tables;
```

```sql
select table_id, column_name, type, label, unit_code, unit_label
from mcd_columns
order by table_id, ordinal;
```

```sql
select table_id, column_name, ordinal
from mcd_primary_keys
order by table_id, ordinal;
```

```sql
select table_id, column_name, ref_table_id, ref_column_name
from mcd_foreign_keys
order by table_id, ordinal;
```

Metadata tables available to agents:

| Table | Purpose |
| --- | --- |
| `mcd_tables` | Manifest table IDs and package source paths. |
| `mcd_columns` | Column names, types, labels, enum values, nullability, and units. |
| `mcd_primary_keys` | Primary key columns in stable key order. |
| `mcd_foreign_keys` | Declared joins between package tables. |
| `mcd_units` | Semantic units for measured columns. |

When answering from SQL, include the table names, column names, filters, join
conditions, and aggregation logic used.

## Reading Markdown

Use `mcd_markdown` when the answer depends on nearby prose or when the user asks
for document content.

Set `expandTables` to `true` when the user wants a prose-oriented view that
includes rendered table content inline. Keep it `false` when you need original
source Markdown.

## Exact Object Access

Use object-specific tools when search or SQL has identified a target:

- `mcd_table` for one table schema and optional rows.
- `mcd_chart` for chart metadata and source rows.
- `mcd_images` for image metadata.
- `mcd_annotations` for comments, review notes, or page/line-filtered notes.
- `mcd_relationships` for declared joins outside SQL.
- `mcd_external_data` for references to external source files or systems.
- `mcd_provenance` for package lineage.

Limit returned rows when possible. Prefer schema and metadata first, then fetch
rows only when needed for the user's question.

## Package Updates

Use mutation tools only when the user asks to create or modify package files.

- `mcd_convert_pdf` converts a PDF into a new `.mcd` package.
- `mcd_init` creates a minimal unpacked package directory.
- `mcd_pack` packages an unpacked directory into a `.mcd` file.
- `mcd_unpack` extracts an existing `.mcd` package without overwriting files.
- `mcd_add_annotation` adds a plain-text annotation to an existing package.
- `mcd_render` writes HTML or Markdown output when `output` is supplied.

After any update, run `mcd_validate` on the resulting package.

## Response Rules for Agents

When answering users:

- Cite stable package paths and source lines when available.
- Name the table IDs and columns used for SQL answers.
- Report units from `mcd_columns` or `mcd_units` for measured values.
- Distinguish prose findings from table-row calculations.
- Say when search found metadata but SQL is required for exact row values.
- Avoid claiming that a package contains a value unless it came from search,
  Markdown, metadata, or SQL results.

For ambiguous questions, first retrieve context with `mcd_search` and schema
metadata. Ask a clarification only when the package contains multiple plausible
interpretations and the next tool call cannot resolve the ambiguity.
