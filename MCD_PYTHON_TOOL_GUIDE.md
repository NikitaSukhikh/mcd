# MCD Python Tool Guide for LLM Agents

This guide shows how to use the Python `mcd` library to inspect, validate, query, and extract information from Markdown CSV Document (`.mcd`) packages.

The PyPI distribution name is `mcdee`; the import package is `mcd`.

```python
import mcd
```

## Core Workflow

Open a package once, then use the document object for all inspection.

```python
from pathlib import Path
import mcd

path = Path("examples/revenue-report/revenue-report.mcd")
doc = mcd.open(path)
```

For agents, prefer this order:

1. Validate the package.
2. Inspect document context and available tables.
3. Use SQL metadata tables to discover columns, keys, relationships, and units.
4. Use SQL queries for table questions.
5. Use schema keys, relationships, external data, and provenance shortcuts when lineage or joins matter.
6. Use direct table/chart/image/annotation APIs when exact object access is needed.
7. Return concise answers with the field names, table names, and condition values used.

## Validate a Package

Use validation before relying on a package.

```python
result = doc.validate()

if not result.valid:
    for diagnostic in result.diagnostics:
        print(diagnostic.code, diagnostic.message, diagnostic.source)
```

Structured form:

```python
validation = doc.validate().as_dict()
```

Result shape:

```python
{
    "valid": True,
    "diagnostics": []
}
```

## Read Markdown

Read the original package Markdown:

```python
markdown = doc.markdown()
```

Read Markdown with package table directives expanded into plain Markdown tables:

```python
expanded = doc.markdown(expand_tables=True)
```

Use `expand_tables=True` when an agent needs a human-readable plain text view. For exact numeric answers, prefer SQL queries instead of parsing expanded Markdown.

## Inspect Document Blocks

Blocks expose the parsed canonical document structure.

```python
blocks = doc.blocks()

for block in blocks:
    print(block.id, block.type, block.source)
```

Convert a block to a dictionary:

```python
block_dict = blocks[0].as_dict()
```

Use blocks when you need source-order content, headings, paragraph text, directives, or source line metadata.

## Agent Context

Use `to_agent_context()` for a compact machine-readable overview of the package.

```python
context = doc.to_agent_context()
```

Omit table rows if the package has large tables:

```python
context = doc.to_agent_context(include_tables=False)
```

Arguments:

```python
context = doc.to_agent_context(
    include_tables=True,
    include_layout=False,
)
```

Recommended agent pattern:

```python
context = doc.to_agent_context(include_tables=False)
print(context.keys())
```

Then query only the tables needed for the task.

## Query Tables with SQL

For table-heavy questions, use SQL. Manifest table IDs are available as SQL table names, and the query runtime also exposes MCD metadata tables for discovering schemas, keys, relationships, and units.

```python
result = doc.query("""
    select count(*) as rows, max(revenue_gbp) as max_revenue
    from revenue
""")
```

Top-level form:

```python
result = mcd.query(
    "examples/revenue-report/revenue-report.mcd",
    "select quarter, revenue_gbp from revenue order by revenue_gbp desc limit 1",
)
```

Read result rows as dictionaries:

```python
rows = result.rows
```

Example:

```python
[
    {"quarter": "Q4", "revenue_gbp": 158250.0}
]
```

Inspect metadata:

```python
print(result.columns)
print(result.row_count)
print(len(result))
```

Get structured output:

```python
data = result.as_dict()
```

Result shape:

```python
{
    "columns": ["quarter", "revenue_gbp"],
    "rows": [{"quarter": "Q4", "revenue_gbp": 158250.0}],
    "rowCount": 1,
}
```

Formatted outputs:

```python
json_text = result.to_json()
csv_text = result.to_csv()
table_text = result.to_table()
```

Use SQL for:

- `count(*)`
- `min(...)`, `max(...)`, `avg(...)`, `sum(...)`
- `where` filters
- `join`
- `group by`
- `order by`
- `limit`
- derived expressions
- schema discovery through `mcd_tables`, `mcd_columns`, `mcd_primary_keys`, `mcd_foreign_keys`, and `mcd_units`
- SQLite table-valued PRAGMA queries such as `pragma_table_info('table_id')` and `pragma_foreign_key_list('table_id')`

MCD metadata tables available in every query:

| Table | Important fields | Use |
| --- | --- | --- |
| `mcd_tables` | `table_id`, `data_path`, `schema_path` | List package tables and source paths. |
| `mcd_columns` | `table_id`, `column_name`, `ordinal`, `type`, `label`, `nullable`, `enum_values`, `unit_code`, `unit_label`, `unit_custom` | Discover exact column names, types, and unit metadata. |
| `mcd_primary_keys` | `table_id`, `column_name`, `ordinal` | Discover stable row identity columns in key order. |
| `mcd_foreign_keys` | `table_id`, `column_name`, `ordinal`, `ref_table_id`, `ref_column_name` | Discover reliable joins between package tables. |
| `mcd_units` | `table_id`, `column_name`, `unit_code`, `unit_label`, `unit_custom` | Inspect semantic units for measured numeric values. |

SQLite constraints are also created for package tables where MCD schemas declare keys, so SQLite PRAGMA introspection can be used from read-only `select` queries.

Examples:

```python
# Count rows.
doc.query("select count(*) as rows from revenue").rows

# Find the largest value.
doc.query("""
    select quarter, revenue_gbp
    from revenue
    order by revenue_gbp desc
    limit 1
""").rows

# Group and rank.
doc.query("""
    select plant_code, count(*) as rows
    from production_quality_measurements
    group by plant_code
    order by rows desc
    limit 5
""").rows

# Join two tables.
doc.query("""
    select c.test_id, c.vehicle_variant, v.body_style
    from chassis_brake_validation_specs c
    join vehicle_variant_configuration_specs v
      on c.vehicle_variant = v.variant_id
    where v.trim_level in ('Sport', 'Performance')
    order by c.stop_distance_100_0_m asc
    limit 5
""").rows

# Discover reliable joins from MCD foreign-key metadata before joining.
doc.query("""
    select table_id, column_name, ref_table_id, ref_column_name
    from mcd_foreign_keys
""").rows

# Inspect primary keys and semantic units.
doc.query("""
    select table_id, column_name, ordinal
    from mcd_primary_keys
    order by table_id, ordinal
""").rows

doc.query("""
    select table_id, column_name, unit_code, unit_label
    from mcd_units
""").rows

# SQLite table-valued PRAGMA introspection also works.
doc.query("select name, pk from pragma_table_info('revenue') where pk > 0").rows
doc.query("select [table], [from], [to] from pragma_foreign_key_list('orders')").rows
```

Queries are read-only. Non-`select` statements are rejected:

```python
doc.query("delete from revenue")  # raises ValueError
```

## Table Access

Use `doc.table(id)` when you need a specific table object.

```python
table = doc.table("revenue")
```

Metadata:

```python
print(table.id)
print(table.source)
print(table.schema.id)
print(table.schema.columns)
```

Plain rows:

```python
rows = table.rows()
```

Plain row values are convenient for display:

```python
rows[0]
```

Typed rows:

```python
typed = table.typed_rows()
```

Typed row values include MCD type metadata:

```python
{
    "revenue_gbp": {
        "type": "decimal",
        "value": "125000"
    }
}
```

Pandas DataFrame:

```python
df = table.dataframe()
```

`dataframe()` requires pandas:

```bash
pip install "mcdee[pandas]"
```

Convert table object to a dictionary:

```python
table_dict = table.as_dict()
```

Agent guidance: for thousands of rows, do not load all rows unless needed. Prefer `doc.query(...)` with filtering, aggregation, ordering, and `limit`.

## Schema Access

Access schema from a table:

```python
schema = doc.table("revenue").schema
```

Commands:

```python
schema.id
schema.primary_key
schema.foreign_keys
schema.columns
schema.as_dict()
```

Use schema columns to discover exact column names before writing SQL.

```python
for column in schema.columns:
    print(column["name"], column["type"], column.get("label"), column.get("unit"))
```

Use keys and foreign keys to build joins without guessing:

```python
print(schema.primary_key)
print(schema.foreign_keys)
```

For all package relationships:

```python
for relationship in doc.relationships():
    print(relationship["tableId"], relationship["columns"], relationship["references"])
```

For SQL-first agents, prefer `mcd_primary_keys` and `mcd_foreign_keys` because relationship discovery and analysis can stay in one query runtime. Use `schema.primary_key`, `schema.foreign_keys`, and `doc.relationships()` when you are already working with Python objects instead of SQL.

## External Data and Provenance

Read manifest-declared external resources:

```python
for item in doc.external_data():
    print(item["id"], item["uri"], item["mediaType"])
```

Read package-level provenance metadata:

```python
provenance = doc.provenance()
if provenance:
    print(provenance.get("sources", []))
    print(provenance.get("activities", []))
```

Use provenance when answering source, lineage, generation, or audit questions. Use external data metadata to identify large source datasets that are intentionally not embedded in the package.

## Chart Access

Use `doc.chart(id)` for chart metadata and chart source rows.

```python
chart = doc.chart("revenue-chart")
```

Commands:

```python
chart.table_id
chart.view_id
chart.placement_ref
chart.view
chart.rows()
chart.layout()
chart.as_dict()
```

Convert chart source rows to pandas:

```python
df = chart.dataframe()
```

Render chart source data as a Markdown table:

```python
markdown_table = chart.to_markdown_table()
```

Use charts when the question asks about a displayed visualization. Use SQL against the source table for exact calculations.

## Table View Access

A chart exposes its table view:

```python
view = doc.chart("revenue-chart").view
```

Commands:

```python
view.id
view.table_id
view.display
view.columns
view.chart
view.layout()
view.as_dict()
```

Use table views to understand which columns a rendered table or chart is meant to display.

## Image Access

Use `doc.image(id)` for image metadata.

```python
image = doc.image("process-diagram")
```

Commands:

```python
image.id
image.asset_path
image.role
image.alt
image.caption
image.intrinsic_size
image.as_dict()
```

Use image metadata for alt text, captions, diagrams, and referenced visual assets. The Python API exposes metadata, not image pixels.

## Annotation Access

List annotations:

```python
annotations = doc.annotations()
```

Get one annotation:

```python
annotation = doc.annotation("review-intro")
```

Commands:

```python
annotation.id
annotation.kind
annotation.status
annotation.body
annotation.labels
annotation.target()
annotation.proposed_change()
annotation.as_dict()
```

Use annotations for reviewer comments, proposed changes, and document-quality tasks.

## PDF Conversion

Convert a PDF file into an MCD package:

```python
doc = mcd.convert_pdf(
    "source.pdf",
    "source.mcd",
    title="Imported PDF",
)
```

Convert PDF bytes to MCD bytes:

```python
pdf_bytes = Path("source.pdf").read_bytes()
mcd_bytes = mcd.pdf_to_mcd_bytes(
    pdf_bytes,
    title="Imported PDF",
    source_filename="source.pdf",
)
Path("source.mcd").write_bytes(mcd_bytes)
```

## Error Handling

Unknown IDs raise `KeyError`:

```python
try:
    table = doc.table("missing_table")
except KeyError as exc:
    print(exc)
```

Invalid packages or invalid SQL raise `ValueError`:

```python
try:
    doc.query("delete from revenue")
except ValueError as exc:
    print(exc)
```

Optional pandas support raises `RuntimeError` if pandas is unavailable:

```python
try:
    df = doc.table("revenue").dataframe()
except RuntimeError:
    print("Install mcdee[pandas] for dataframe support.")
```

## Recommended Agent Recipes

### Answer a Numeric Table Question

```python
import mcd

doc = mcd.open("document.mcd")
result = doc.query("""
    select count(*) as matching_rows
    from table_id
    where status = 'approved'
""")
answer = result.rows[0]["matching_rows"]
```

Do not fetch all rows and count manually unless the table is small.

### Find a Maximum or Minimum Row

```python
result = doc.query("""
    select item_id, metric_value
    from measurements
    order by metric_value desc
    limit 1
""")
```

Return both the ID and the metric value.

### Inspect Unknown Tables

For SQL-first discovery:

```python
doc.query("""
    select table_id, column_name, type, label, nullable, unit_code, unit_label
    from mcd_columns
    order by table_id, ordinal
""").rows
```

```python
context = doc.to_agent_context(include_tables=False)

for table_summary in context.get("tables", []):
    print(table_summary)
```

If a table ID is known:

```python
table = doc.table("revenue")
for column in table.schema.columns:
    print(column["name"], column["type"])
```

### Join Related Tables

First discover relationships:

```python
doc.query("""
    select table_id, column_name, ref_table_id, ref_column_name
    from mcd_foreign_keys
    where table_id = 'table_a'
""").rows
```

Then join using the discovered columns:

```python
result = doc.query("""
    select a.id, b.category, a.score
    from table_a a
    join table_b b on a.foreign_id = b.id
    where b.category = 'target'
    order by a.score desc
    limit 10
""")
```

### Cite Source Fields in Final Answers

When answering, include the table and column names used:

```text
The highest revenue quarter is Q4 with revenue_gbp = 158250.0, from table `revenue`.
```

## API Summary

Top-level functions:

```python
mcd.open(path) -> Document
mcd.query(path, sql) -> QueryResult
mcd.convert_pdf(input, output, title=None) -> Document
mcd.pdf_to_mcd_bytes(pdf, title=None, source_filename=None) -> bytes
```

Document:

```python
doc.path
doc.validate()
doc.blocks()
doc.table(id)
doc.chart(id)
doc.image(id)
doc.annotation(id)
doc.annotations()
doc.external_data()
doc.provenance()
doc.relationships()
doc.markdown(expand_tables=False)
doc.query(sql)
doc.to_agent_context(include_tables=True, include_layout=False)
```

QueryResult:

```python
result.columns
result.rows
result.row_count
len(result)
result.values()
result.as_dict()
result.to_json()
result.to_csv()
result.to_table()
```

Table:

```python
table.id
table.source
table.schema
table.rows()
table.typed_rows()
table.dataframe()
table.as_dict()
```

TableSchema:

```python
schema.id
schema.primary_key
schema.foreign_keys
schema.columns
schema.as_dict()
```

Chart:

```python
chart.table_id
chart.view_id
chart.placement_ref
chart.view
chart.rows()
chart.dataframe()
chart.to_markdown_table()
chart.layout()
chart.as_dict()
```

TableView:

```python
view.id
view.table_id
view.display
view.columns
view.chart
view.layout()
view.as_dict()
```

Image:

```python
image.id
image.asset_path
image.role
image.alt
image.caption
image.intrinsic_size
image.as_dict()
```

Annotation:

```python
annotation.id
annotation.kind
annotation.status
annotation.body
annotation.labels
annotation.target()
annotation.proposed_change()
annotation.as_dict()
```
