# mcd

MCD is a Markdown CSV Document format. It stores document prose as Markdown, meaningful tables as typed CSV data, and rendering metadata as package files that machines can inspect without reverse-engineering a PDF.

The `mcd` CLI works with `.mcd` files as packages, and it can also validate and render a plain Markdown file saved with a `.mcd` extension as a minimal document.

## Install and Run

For command-line use, install the Rust CLI:

```bash
cargo install mcd-cli --version 0.1.0-alpha.0
```

Then use:

```bash
mcd <command>
```

From this repository:

```bash
cargo run -p mcd-cli -- <command>
```

To install the CLI locally from the checkout:

```bash
cargo install --path crates/mcd-cli
```

Rust libraries are available as crates:

```bash
cargo add mcd-core@0.1.0-alpha.0
cargo add mcd-render@0.1.0-alpha.0
```

TypeScript/JavaScript projects can install:

```bash
npm install @mcd-nix/parser
```

Python projects can install the PyPI distribution `mcdee`, which exposes
the import package `mcd`:

```bash
pip install mcdee
```

PHP projects can install the Composer package. The PHP wrapper delegates to the
`mcd` CLI, so install the CLI first and make sure it is on `PATH`.

```bash
composer require mcd-nix/parser
```

The examples below use the installed `mcd` command. If you have not installed
it, replace `mcd` with `cargo run -p mcd-cli --`.

A dedicated command list is available in [CLI_COMMANDS.md](CLI_COMMANDS.md).
For agent-oriented instructions on creating a package from scratch, see [MCD_CREATION_GUIDE.md](MCD_CREATION_GUIDE.md).
For publishing and downloadable binary releases, see [RELEASE.md](RELEASE.md).

## Language Bindings

- Python bindings are in `bindings/python`.
- TypeScript/JavaScript bindings are in `bindings/typescript`.
- PHP wrapper bindings are in `bindings/php` and delegate to the installed `mcd` CLI.
- A local-first browser viewer/editor is in `web/mcd-viewer`.

## MCD Package Layout

An MCD package is a ZIP-style `.mcd` file with safe internal paths. A minimal unpacked package looks like this:

```text
unpacked/
  mimetype
  manifest.json
  content/
    main.md
```

The root `mimetype` file contains:

```text
application/vnd.mcd+zip
```

The root `manifest.json` points to the Markdown entrypoint:

```json
{
  "format": "MCD",
  "version": "0.1",
  "profile": "MCD-Core",
  "entrypoint": "content/main.md"
}
```

Packages with tables usually add:

```text
tables/<table-id>.csv
tables/<table-id>.schema.json
tables/<table-id>.view.json
```

Markdown places those tables with directives:

```markdown
:::table
ref: revenue-table
table: revenue
view: default
display: table
caption: Revenue by quarter
:::
```

Images and annotations are also stored as package metadata and assets, then referenced from Markdown or the manifest.

## Common Workflows

Create, pack, and validate a new document:

```bash
mcd init work/report
mcd pack work/report --output report.mcd
mcd validate report.mcd
```

Render a package for reading:

```bash
mcd render report.mcd --html --output report.html
mcd render report.mcd --markdown --output report.rendered.md
```

Run the browser viewer/editor:

```bash
cd web/mcd-viewer
npm install
npm run dev
```

The web app opens `.mcd` files locally in the browser, validates through the
WASM TypeScript binding, previews expanded Markdown, and edits text,
annotations, and CSV-backed table rows.

Unpack a package, edit its source files, then repack it:

```bash
mcd unpack report.mcd --output work/report
mcd pack work/report --output report.updated.mcd
mcd validate report.updated.mcd
```

Extract machine-readable content:

```bash
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --json
mcd extract report.mcd --tables
mcd extract report.mcd --images
mcd extract report.mcd --charts
mcd extract report.mcd --annotations
```

## Command Reference

### `mcd inspect`

Prints a JSON summary of a package.

```bash
mcd inspect <file.mcd>
```

Example:

```bash
mcd inspect examples/minimal/minimal.mcd
```

Output includes the format, version, profile, entrypoint path, table count, annotation count, and package entry count.

### `mcd validate`

Validates a package or plain Markdown `.mcd` file.

```bash
mcd validate [--format text|json] <file.mcd>
```

Examples:

```bash
mcd validate report.mcd
mcd validate report.mcd --format json
```

Text output prints `valid` on success. JSON output prints structured diagnostics. Validation failures exit with a non-zero status.

### `mcd extract`

Extracts one kind of content to stdout. Choose exactly one extraction mode.

```bash
mcd extract <file.mcd> --json
mcd extract <file.mcd> --markdown
mcd extract <file.mcd> --markdown --expand-tables
mcd extract <file.mcd> --tables
mcd extract <file.mcd> --images
mcd extract <file.mcd> --charts
mcd extract <file.mcd> --annotations
```

Modes:

| Option | Output |
| --- | --- |
| `--json` | Canonical JSON export for the package content. |
| `--markdown` | Original Markdown entrypoint content. |
| `--markdown --expand-tables` | Markdown with table directives expanded as Markdown tables. |
| `--tables` | JSON table metadata and row data. |
| `--images` | JSON image metadata. |
| `--charts` | JSON chart metadata and source data. |
| `--annotations` | JSON annotation metadata. |
| `--export annotations` | Alias for annotation export. |

Annotation export can be filtered:

```bash
mcd extract report.mcd --annotations --page content/main.md
mcd extract report.mcd --annotations --page content/main.md --line 12
mcd extract report.mcd --export annotations --page content/main.md --line 12
```

`--page` and `--line` only apply to annotation export. Lines are 1-based.

### `mcd render`

Renders a package to HTML or expanded Markdown.

```bash
mcd render <file.mcd> --html --output <path>
mcd render <file.mcd> --markdown --output <path>
```

HTML examples:

```bash
mcd render report.mcd --html --output report.html
mcd render report.mcd --html --output render/report
```

When the HTML output path is a directory or has no file extension, the renderer writes an HTML project:

```text
render/report/
  index.html
  styles.css
  assets/
```

When the HTML output path has a file extension, the renderer writes a standalone HTML file.

Markdown example:

```bash
mcd render report.mcd --markdown --output report.rendered.md
```

Markdown rendering expands package-backed tables and chart metadata into a plain Markdown projection.

### `mcd pack`

Packs an unpacked directory into a `.mcd` package.

```bash
mcd pack <directory> --output <file.mcd>
```

Example:

```bash
mcd pack examples/revenue-report/unpacked --output revenue-report.mcd
```

If the source directory does not contain a root `mimetype` file, `pack` writes the standard MCD mimetype entry automatically. The `mimetype` entry is stored first and uncompressed; other files are compressed.

### `mcd unpack`

Unpacks a `.mcd` package into a directory.

```bash
mcd unpack <file.mcd> --output <directory>
```

Example:

```bash
mcd unpack revenue-report.mcd --output work/revenue-report
```

The output path must be a directory. Existing files are not overwritten. Unsafe archive paths, such as paths that escape the output directory, are rejected.

### `mcd init`

Initializes a minimal unpacked MCD directory.

```bash
mcd init <directory>
```

Example:

```bash
mcd init work/new-report
```

This creates:

```text
work/new-report/
  mimetype
  manifest.json
  content/
    main.md
```

The generated Markdown starts with `# Untitled`.

### `mcd add-annotation`

Adds a plain-text annotation to an existing package in place.

```bash
mcd add-annotation <file.mcd> <text> --page <package-path> [--line <line>] [--id <id>]
```

Examples:

```bash
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md --line 18
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md --line 18 --id review-intro
```

Rules:

| Option | Meaning |
| --- | --- |
| `<text>` | Annotation body. It cannot be empty. |
| `--page` | Required package path to target, for example `content/main.md`. The path must exist in the package. |
| `--line` | Optional 1-based line number in the target page. |
| `--id` | Optional stable ID. If omitted, the CLI generates `annotation-0001`, `annotation-0002`, and so on. |

The command prints the annotation ID on success, updates `manifest.json`, writes `annotations/<id>.annotation.json`, and validates the package before returning.

### `mcd convert-pdf`

Converts a PDF into a minimal MCD package.

```bash
mcd convert-pdf <file.pdf> --output <file.mcd> [--title <title>]
```

Examples:

```bash
mcd convert-pdf source.pdf --output source.mcd
mcd convert-pdf source.pdf --output source.mcd --title "Imported PDF"
```

The converter extracts PDF text into Markdown, embeds the original PDF under `assets/`, and writes a valid package. `--title` controls the generated Markdown heading; when omitted, the converter derives a title from the input filename.

## Examples in This Repository

The repository includes ready-made packages:

```bash
mcd inspect examples/minimal/minimal.mcd
mcd validate examples/revenue-report/revenue-report.mcd
mcd extract examples/revenue-report/revenue-report.mcd --tables
mcd extract examples/visual-report/visual-report.mcd --images
mcd render examples/revenue-report/revenue-report.mcd --html --output target/revenue-report.html
```

The `examples/*/unpacked` directories show the source layout before packaging.

## Notes for Automation

- CLI extraction commands write data to stdout, so they can be redirected into files or piped into other tools.
- Render, pack, unpack, annotation, and PDF conversion commands write files and print nothing on success, except `add-annotation`, which prints the created annotation ID.
- Commands reject ambiguous mode selections, such as `mcd extract report.mcd --json --tables`.
- Internal package paths use forward slashes, for example `content/main.md`, even on Windows.
- Keep canonical table data in CSV plus schema files. Markdown pipe tables are suitable for prose, but external typed CSV tables are the machine-readable source of truth.

More design background is available in [ABOUT.md](ABOUT.md) and rendering notes are in [Rendering_MCD.md](Rendering_MCD.md).
