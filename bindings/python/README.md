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
