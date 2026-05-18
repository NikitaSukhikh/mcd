For `.mcd`, images and charts should be handled with one rule:

> **Anything visual is allowed, but anything meaningful must also be represented as Markdown text, typed table data, or machine-readable layout/view metadata.**

That keeps the format narrow: no “modules,” no domain-specific semantic systems.

## 1. Images in `.mcd`

Images should be stored as assets, not as semantic content.

Recommended structure:

```text
report.mcd
  manifest.json
  content/
    main.md
  tables/
    revenue.csv
    revenue.schema.json
  assets/
    factory-photo.jpg
    logo.svg
    process-diagram.svg
  images/
    factory-photo.image.json
    logo.image.json
    process-diagram.image.json
  layout/
    styles.json
    page-map.json
```

In Markdown:

```markdown
# Manufacturing overview

The facility layout is shown below.

:::image
ref: facility-layout-image
asset: process-diagram
caption: Facility layout showing intake, processing, quality control, and dispatch.
alt: Diagram of the facility workflow from intake to dispatch.
:::
```

Then `images/process-diagram.image.json`:

```json
{
  "id": "process-diagram",
  "asset": "assets/process-diagram.svg",
  "mediaType": "image/svg+xml",
  "role": "informative",
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

SVG should be the preferred image format for diagrams, icons, and technical graphics because SVG is an XML-based language for describing two-dimensional vector and mixed vector/raster graphics, and it can scale across resolutions. ([W3C][1]) Raster formats such as PNG, JPEG, and WebP can still be allowed for photos.

## Image roles

Each image should have a declared role:

```text
decorative
informative
diagram
photo
logo
rendered-table-prohibited
rendered-text-prohibited
```

Example decorative image:

```json
{
  "id": "cover-background",
  "asset": "assets/cover-bg.jpg",
  "mediaType": "image/jpeg",
  "role": "decorative",
  "alt": "",
  "hash": "sha256:..."
}
```

Example informative image:

```json
{
  "id": "factory-photo",
  "asset": "assets/factory-photo.jpg",
  "mediaType": "image/jpeg",
  "role": "informative",
  "caption": "Main production floor in April 2026.",
  "alt": "Photo of the main production floor with three assembly lines.",
  "hash": "sha256:..."
}
```

W3C accessibility guidance treats non-text content as needing a text alternative that serves the equivalent purpose, with exceptions for cases like decorative content. ([W3C][2]) `.mcd` should adopt the same basic rule, but make it part of machine-readability validation.

## Critical image rule

Images must not be the only carrier of meaningful text or table data.

Bad `.mcd`:

```markdown
:::image
asset: scanned-financial-table
caption: Revenue by quarter
:::
```

Good `.mcd`:

```markdown
:::table
ref: revenue-table
table: revenue
view: default
caption: Revenue by quarter
:::

:::image
ref: revenue-table-screenshot
asset: revenue-table-screenshot
caption: Screenshot of the rendered revenue table.
role: illustrative
:::
```

Validation rule:

```text
If an image contains meaningful text, numbers, or table-like data, that content must also exist in Markdown or CSV.
```

So an image can show something visually, but it cannot be the canonical source of text or data.

## 2. Charts and graphs with numbers

Charts should not be stored as standalone images.

In `.mcd`, a chart should be a **view of a typed CSV table**.

The data lives here:

```text
tables/revenue.csv
tables/revenue.schema.json
```

The chart view lives here:

```text
tables/revenue.chart.view.json
```

The Markdown anchor places it in the document:

```markdown
# Revenue growth

Revenue increased across the year.

:::table
ref: revenue-chart
table: revenue
view: quarterly-bar-chart
display: chart
caption: Revenue by quarter
:::
```

This keeps the model simple:

```text
A chart is not a new semantic object.
A chart is a rendered view of a table.
```

## Example table

`tables/revenue.csv`:

```csv
quarter,revenue_gbp,margin_percent
Q1,120000,18.4
Q2,132000,19.1
Q3,141000,20.0
Q4,169000,22.7
```

`tables/revenue.schema.json`:

```json
{
  "id": "revenue",
  "columns": [
    {
      "name": "quarter",
      "type": "string",
      "label": "Quarter"
    },
    {
      "name": "revenue_gbp",
      "type": "decimal",
      "unit": "GBP",
      "label": "Revenue"
    },
    {
      "name": "margin_percent",
      "type": "decimal",
      "unit": "percent",
      "label": "Margin"
    }
  ]
}
```

## Example chart view

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
    },
    "markLabels": {
      "show": true,
      "format": "currency",
      "currency": "GBP"
    }
  },
  "style": {
    "width": "160mm",
    "height": "90mm",
    "colors": {
      "bars": "#2f5f98",
      "axis": "#333333",
      "grid": "#dddddd"
    }
  }
}
```

The renderer can output an SVG chart, but the machine-readable truth is still:

```text
CSV table + schema + chart view JSON
```

Vega-Lite is a relevant design precedent because it uses declarative JSON specifications for visualizations, but `.mcd` should probably use a smaller fixed subset at first rather than adopting the full Vega-Lite language. ([Vega][3])

## Generated chart asset

The rendered chart may be stored as SVG:

```text
rendered/
  revenue-chart.svg
```

But it must be marked as generated:

```json
{
  "id": "revenue-chart-rendered",
  "type": "generated-rendering",
  "source": {
    "table": "revenue",
    "view": "quarterly-bar-chart"
  },
  "asset": "rendered/revenue-chart.svg",
  "mediaType": "image/svg+xml",
  "hash": "sha256:..."
}
```

The rendered SVG is useful for humans. The table and chart view are useful for machines.

## Chart validation rules

A chart view should be valid only if:

```text
The referenced table exists.
The referenced columns exist.
The referenced columns have valid types.
The axis labels match the schema/view.
The displayed numbers are generated from table values.
The rendered chart asset, if present, matches the chart view.
The page map links the chart to the Markdown anchor.
```

Example page map entry:

```json
{
  "page": 3,
  "objects": [
    {
      "type": "chart",
      "ref": "revenue-chart",
      "source": "content/main.md:7-13",
      "table": "revenue",
      "view": "quarterly-bar-chart",
      "bbox": [72, 180, 510, 420],
      "renderedAsset": "rendered/revenue-chart.svg"
    }
  ]
}
```

## Charts should be extractable as tables

The parser should expose charts like this:

```python
doc = mcd.open("report.mcd")

chart = doc.chart("revenue-chart")

chart.table_id
# "revenue"

chart.view
# "quarterly-bar-chart"

chart.dataframe()
# exact source data

chart.to_markdown_table()
# human/LLM-readable table

chart.layout()
# page, bbox, styles
```

An AI agent should never have to infer values from chart pixels.

Bad:

```text
Read bar heights from image.
```

Good:

```text
Read chart.source.table = revenue.
Read revenue.csv.
Read chart.view.y = revenue_gbp.
```

## Recommended Markdown syntax

Use one directive family:

```markdown
:::table
...
:::
```

and let `display` decide whether it appears as a table or chart.

### Render as normal table

```markdown
:::table
ref: revenue-table
table: revenue
view: default
display: table
caption: Revenue by quarter
:::
```

### Render as chart

```markdown
:::table
ref: revenue-chart
table: revenue
view: quarterly-bar-chart
display: chart
caption: Revenue by quarter
:::
```

This avoids adding a separate `:::chart` object type. Internally, both are table placements.

## Handling diagrams

Under the narrowed `.mcd` model, diagrams are images unless they can be expressed as text or tables.

Allowed:

```markdown
:::image
ref: process-diagram
asset: process-diagram
caption: Process diagram for invoice approval.
alt: Invoice approval process: submit invoice, validate supplier, approve manager, schedule payment.
:::
```

If the diagram has structured information, represent it as a table too:

```markdown
:::table
ref: approval-flow-table
table: approval-flow
view: default
caption: Invoice approval flow
:::

:::image
ref: approval-flow-diagram
asset: approval-flow-diagram
caption: Visual diagram of the invoice approval flow.
alt: Visual diagram of the approval flow described in the table above.
:::
```

`tables/approval-flow.csv`:

```csv
step,actor,action,next_step
1,Requester,Submit invoice,2
2,Finance,Validate supplier,3
3,Manager,Approve invoice,4
4,Finance,Schedule payment,
```

The image is visual. The table is machine-readable.

## Image and chart conformance levels

Add this to `.mcd` validation:

```text
MCD-Core
  Markdown text, typed tables, layout, page map.

MCD-Images
  Images allowed with declared role, alt text, caption, dimensions, hash.

MCD-Charts
  Charts allowed only as table-backed views.

MCD-Strict
  No meaningful text/table/number content may exist only inside an image.
```

A fully machine-readable document should claim:

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

## Best rule set

For `.mcd`, use these rules:

```text
1. Images are assets with metadata.
2. Every image has a role: decorative or informative.
3. Informative images require caption and alt text.
4. Images may not be the only source of meaningful text, numbers, or tables.
5. SVG is preferred for diagrams and generated visuals.
6. Scripts, external resources, and active behavior in SVG are disallowed.
7. Charts are table views, not standalone image objects.
8. Chart numbers always come from CSV data.
9. Chart visual rules live in view JSON.
10. Rendered chart SVG/PNG is optional and generated, not canonical.
```

This preserves the `.mcd` philosophy:

```text
Text lives in Markdown.
Numbers live in CSV.
Meaningful tables live in CSV + schema.
Charts are views of tables.
Images are visual assets with metadata.
Layout lives in JSON.
```

[1]: https://www.w3.org/TR/SVG2/?utm_source=chatgpt.com "Scalable Vector Graphics (SVG) 2"
[2]: https://www.w3.org/TR/WCAG21/?utm_source=chatgpt.com "Web Content Accessibility Guidelines (WCAG) 2.1"
[3]: https://vega.github.io/vega-lite/?utm_source=chatgpt.com "A High-Level Grammar of Interactive Graphics | Vega-Lite"
