# mcd Python bindings

Rust-backed Python bindings for Markdown CSV Document packages.

Install the PyPI distribution:

```bash
pip install mcdee
```

Released builds include prebuilt wheels for common Windows, macOS, and Linux
machines. To require a wheel and avoid local Rust/C compiler builds:

```bash
pip install --only-binary=:all: mcdee
```

The distribution name is `mcdee`; the Python import package is `mcd`.

```python
import mcd

doc = mcd.open("report.mcd")
validation = doc.validate()
blocks = doc.blocks()
table = doc.table("revenue")
markdown = doc.markdown(expand_tables=True)
external_data = doc.external_data()
provenance = doc.provenance()
relationships = doc.relationships()
```

Use `query()` for SQLite-backed table analysis. Package table IDs are available
as SQL table names, and MCD schema metadata is exposed through runtime tables:

```python
doc.query("select table_id, column_name from mcd_primary_keys").rows
doc.query("""
    select table_id, column_name, ref_table_id, ref_column_name
    from mcd_foreign_keys
""").rows
doc.query("select table_id, column_name, unit_code from mcd_units").rows
```

Use `queries()` when an agent needs several independent result sets and the
package should be loaded only once:

```python
results = doc.queries([
    "select count(*) as rows from revenue",
    "select max(revenue_gbp) as max_revenue from revenue",
])
```

SQLite PRAGMA introspection works through read-only `select` queries:

```python
doc.query("select name, pk from pragma_table_info('revenue') where pk > 0").rows
```

## MCP Server

The official MCP server is the Rust `mcd-mcp` binary in `crates/mcd-mcp`. Use
that for the runtime-neutral published server.

This Python package also provides a convenience MCP server for Python-first
environments. Install the optional MCP dependencies when you want an AI agent to
call MCD tools through the Python bindings:

```bash
pip install "mcdee[mcp]"
```

Run the server over stdio:

```bash
mcd-python-mcp
```

Equivalent module form:

```bash
python -m mcd.mcp_server
```

The server exposes these tools:

| Tool | Purpose |
| --- | --- |
| `mcd_validate` | Validate a `.mcd` package and return diagnostics. |
| `mcd_agent_context` | Return a compact machine-readable document overview. |
| `mcd_markdown` | Read Markdown, optionally expanding table directives. |
| `mcd_query` | Run read-only SQL against package tables and metadata. |
| `mcd_queries` | Run multiple read-only SQL queries against one loaded package. |
| `mcd_table` | Return table schema and optional row data. |
| `mcd_chart` | Return chart metadata and optional source rows. |
| `mcd_image` | Return image metadata. |
| `mcd_annotations` | Return annotation metadata, optionally filtered. |
| `mcd_relationships` | Return declared table relationships. |
| `mcd_external_data` | Return manifest-declared external data references. |
| `mcd_provenance` | Return package provenance metadata. |
| `mcd_convert_pdf` | Convert a PDF into an MCD package. |

Example MCP client configuration for a local stdio server:

```json
{
  "mcpServers": {
    "mcd": {
      "command": "mcd-python-mcp"
    }
  }
}
```
