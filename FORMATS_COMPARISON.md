Below is a comparison of `.mcd` against the closest existing formats.

Legend:

```text
✓  strong/native support
◐  partial, indirect, or tool-dependent support
✕  not a native goal
```

| Format                             | Canonical content model                                  | Human-readable source | Human-rendered view | Machine/AI readability |   Typed tables | Images |     Charts/graphs with numbers |                                 Annotations | Layout/page metadata | Main gap vs `.mcd`                                                             |
| ---------------------------------- | -------------------------------------------------------- | --------------------: | ------------------: | ---------------------: | -------------: | -----: | -----------------------------: | ------------------------------------------: | -------------------: | ------------------------------------------------------------------------------ |
| **`.mcd` — Markdown CSV Document** | Markdown + CSV + schemas + assets + layout/page map JSON |                     ✓ |                   ✓ |                      ✓ |              ✓ |      ✓ |       ✓, as table-backed views | ✓, anchored to Markdown/table/image regions |                    ✓ | Best Overall                                                      |
| **PDF**                            | Visual/page-description file                             |                     ✕ |                   ✓ |                ◐ / low |              ✕ |      ✓ |         ◐, usually visual only |                                           ✓ |  ✓, but visual-first | Machine-readable structure is not canonical; extraction is often heuristic     |
| **Tagged PDF / PDF/UA**            | PDF plus semantic tags                                   |                     ✕ |                   ✓ |                      ◐ |              ◐ |      ✓ |                              ◐ |                                           ✓ |                    ✓ | Better than PDF, but still page/PDF-first, not Markdown/data-first             |
| **EPUB 3**                         | ZIP package with XHTML/SVG/CSS/assets, manifest, spine   |                     ◐ |                   ✓ |                      ✓ | ◐, HTML tables |      ✓ |                              ◐ |                                           ◐ |                    ◐ | Good package model, but no mandatory typed CSV tables or Markdown anchors      |
| **Quarto**                         | Markdown/notebooks rendered to many outputs              |                     ✓ |                   ✓ |                      ✓ |              ◐ |      ✓ | ◐ / ✓ with code/data workflows |                                           ◐ |                    ◐ | Publishing system, not a strict portable file format                           |
| **MyST Markdown**                  | Structured Markdown for scientific/technical publishing  |                     ✓ |                   ✓ |                      ✓ |              ◐ |      ✓ |                              ◐ |                                           ◐ |                    ◐ | Strong Markdown model, but not a strict `.mcd`-style CSV+schema package        |
| **JATS**                           | XML article structure                                    |                     ✕ |                   ◐ |                      ✓ |              ◐ |      ✓ |                              ◐ |                           ✓, notes/metadata |                    ◐ | Very machine-readable, but XML-heavy and scholarly-domain-oriented             |
| **TEI**                            | XML text encoding                                        |                     ✕ |                   ◐ |                      ✓ |              ◐ |      ✓ |                              ◐ |                                          ✓✓ |                    ◐ | Excellent for rich text annotation, but too complex and not table/layout-first |
| **ODF**                            | XML office document package                              |                     ✕ |                   ✓ |                      ◐ |              ◐ |      ✓ |                              ✓ |                                           ✓ |                    ✓ | Office-oriented, complex, not Markdown/CSV-native                              |
| **OOXML / DOCX**                   | XML office document package                              |                     ✕ |                   ✓ |                      ◐ |              ◐ |      ✓ |                              ✓ |                                           ✓ |                    ✓ | Widely supported, but complex and not cleanly AI-readable without tooling      |
| **CSVW**                           | CSV + metadata/schema                                    |                     ✓ |                   ✕ |                      ✓ |              ✓ |      ✕ |                              ✕ |                                           ◐ |                    ✕ | Excellent table layer, not a document format                                   |
| **Frictionless Data Package**      | Data package + table schemas + CSV/JSON                  |                     ✓ |                   ✕ |                      ✓ |              ✓ |      ✕ |                              ✕ |                                           ◐ |                    ✕ | Excellent dataset package, not a human document format                         |
| **Web Annotation**                 | JSON-LD annotation model                                 |                     ◐ |                   ✕ |                      ✓ |              ✕ |      ◐ |                              ✕ |                                          ✓✓ |                    ✕ | Annotation layer only, not a document format                                   |
| **IIIF Presentation**              | Image/canvas/annotation presentation model               |                     ✕ |                   ✓ |                      ✓ |              ✕ |     ✓✓ |                              ✕ |                                          ✓✓ |                    ✓ | Excellent for image-heavy documents, not Markdown/table-first                  |
| **Vega-Lite**                      | Declarative JSON visualization spec                      |                     ◐ |                   ✓ |                      ✓ |              ◐ |      ✕ |                             ✓✓ |                                           ✕ |                    ◐ | Excellent chart model, not a full document format                              |

PDF is standardized through the ISO 32000 family, while PDF/UA adds requirements around tagged PDF and semantic structure for accessibility; however, PDF/UA itself notes that Tagged PDF is the semantic layer inside a PDF file, not a separate canonical Markdown/data source. ([pdfa.org][1]) EPUB is the closest packaging analogue: it is ZIP-based, uses a manifest/spine, and its content is built on XHTML/SVG plus resources such as CSS and images. ([W3C][2]) Quarto and MyST are closest on the authoring side because they use Markdown-style sources for publishing technical/scientific documents to multiple outputs. ([Quarto][3]) JATS and TEI are closest on strict machine-readable document structure, but both are XML-first rather than Markdown/CSV-first. ([NISO][4])

## More focused comparison against `.mcd` goals

| Goal                             | `.mcd`                                                         | Best existing analogue                | Why `.mcd` is different                                                          |
| -------------------------------- | -------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------- |
| **Simple human-readable source** | Markdown                                                       | CommonMark / MyST / Quarto            | `.mcd` would make Markdown the canonical text layer, not just an authoring input |
| **Typed table data**             | CSV + schema                                                   | CSVW / Frictionless Table Schema      | `.mcd` would require tables to be anchored inside the Markdown document flow     |
| **Human page rendering**         | HTML/CSS/PDF-like render                                       | PDF / EPUB / Quarto                   | `.mcd` rendering would be generated from Markdown+CSV, not reverse-extracted     |
| **Images**                       | Asset + metadata + alt/caption                                 | EPUB / HTML / IIIF                    | `.mcd` would prohibit meaningful text/numbers existing only inside images        |
| **Charts**                       | View of typed table                                            | Vega-Lite                             | `.mcd` would treat charts as renderings of CSV-backed tables                     |
| **Annotations**                  | JSON annotations targeting Markdown/table/image spans          | W3C Web Annotation                    | `.mcd` would bind annotations directly to Markdown source spans and table cells  |
| **AI readability**               | Native parser reads Markdown, CSV, schema, annotations, layout | Quarto/MyST + CSVW                    | `.mcd` would avoid opaque office/XML/PDF internals                               |
| **Layout traceability**          | Page map from rendered object to source anchor                 | PDF layout / EPUB spine / IIIF canvas | `.mcd` would make source-to-render mapping explicit and verifiable               |

CommonMark is relevant because it aims to standardize Markdown with an unambiguous syntax and test suite. ([commonmark.org][5]) CSVW and Frictionless are relevant because they address the weakness of raw CSV: CSV alone lacks column types and validation metadata, while Frictionless Table Schema provides a language-agnostic way to declare schemas for tabular data. ([W3C][6]) Web Annotation is relevant because it defines a standard annotation model with bodies and targets, and IIIF is relevant because it uses annotations to associate images, text, and other resources with canvases. ([W3C][7]) Vega-Lite is relevant because it already models charts as declarative JSON mappings from data to graphical marks. ([Vega][8])

## Short conclusion

The closest overall stack is:

```text
EPUB packaging
+ CommonMark/MyST-style Markdown
+ CSVW/Frictionless-style table schemas
+ Vega-Lite-style chart views
+ Web Annotation-style annotations
+ PDF-like rendering
```

But no existing format has the exact `.mcd` rule set:

```text
Text lives in Markdown.
Tables live in CSV + schema.
Charts are views of tables.
Images are assets with metadata.
Annotations target exact Markdown/table/image locations.
Layout is machine-readable.
Rendered pages map back to source.
```

[1]: https://pdfa.org/resource/iso-32000-2/ "ISO 32000-2 – PDF Association"
[2]: https://www.w3.org/TR/epub-33/ "EPUB 3.3"
[3]: https://quarto.org/ "Quarto"
[4]: https://www.niso.org/standards-committees/jats "Standardized Markup for Journal Articles: Journal Article Tag Suite (JATS) | NISO website"
[5]: https://commonmark.org/ "CommonMark"
[6]: https://www.w3.org/TR/tabular-data-primer/ "CSV on the Web: A Primer"
[7]: https://www.w3.org/TR/annotation-model/ "Web Annotation Data Model"
[8]: https://vega.github.io/vega-lite/ "A High-Level Grammar of Interactive Graphics | Vega-Lite"
