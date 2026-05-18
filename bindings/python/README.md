# mcd Python bindings

Rust-backed Python bindings for Markdown CSV Document packages.

```python
import mcd

doc = mcd.open("report.mcd")
validation = doc.validate()
blocks = doc.blocks()
table = doc.table("revenue")
markdown = doc.markdown(expand_tables=True)
```
