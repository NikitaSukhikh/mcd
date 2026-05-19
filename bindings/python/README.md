# mcd Python bindings

Rust-backed Python bindings for Markdown CSV Document packages.

Install the PyPI distribution:

```bash
pip install mcdee
```

The distribution name is `mcdee`; the Python import package is `mcd`.

```python
import mcd

doc = mcd.open("report.mcd")
validation = doc.validate()
blocks = doc.blocks()
table = doc.table("revenue")
markdown = doc.markdown(expand_tables=True)
```
