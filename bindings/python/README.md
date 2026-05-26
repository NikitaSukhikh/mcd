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

SQLite PRAGMA introspection works through read-only `select` queries:

```python
doc.query("select name, pk from pragma_table_info('revenue') where pk > 0").rows
```
