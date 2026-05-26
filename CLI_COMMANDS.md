# MCD CLI Commands

This file lists the available `mcd` command line commands and their options.
For a step-by-step guide to creating an unpacked package and packing it into a `.mcd` file, see [MCD_CREATION_GUIDE.md](MCD_CREATION_GUIDE.md).

Run from this repository with:

```bash
cargo run -p mcd-cli -- <command>
```

After installing the CLI, use:

```bash
mcd <command>
```

## Quick Render

1. Render an MCD file to HTML:

```bash
mcd render report.mcd --html --output report.html
```

For a repository example:

```bash
mcd render examples/revenue-report/revenue-report.mcd --html --output revenue-report.html
```

2. Open the generated HTML file in your browser:

```bash
start revenue-report.html
```

On macOS or Linux:

```bash
open revenue-report.html
xdg-open revenue-report.html
```

## Commands

| Command | Purpose |
| --- | --- |
| `mcd inspect <file>` | Inspect an MCD package and print a JSON summary. |
| `mcd add-annotation <file> <text> --page <page>` | Add a plain-text annotation to an MCD package. |
| `mcd convert-pdf <file> --output <output>` | Convert a PDF into a minimal MCD package. |
| `mcd validate <file>` | Validate an MCD package. |
| `mcd extract <file> <mode>` | Extract content from an MCD package. |
| `mcd query <file> <sql>` | Query package tables with read-only SQL. |
| `mcd tools [file]` | Show Python, SQL, schema, relationship, unit, external-data, and provenance capabilities for agents. |
| `mcd render <file> <target> --output <output>` | Render an MCD package. |
| `mcd pack <directory> --output <output>` | Pack an unpacked directory into an MCD package. |
| `mcd unpack <file> --output <directory>` | Unpack an MCD package into a directory. |
| `mcd init <directory>` | Initialize a minimal unpacked MCD directory. |
| `mcd help [command]` | Print global or command-specific help. |

## Global Options

```bash
mcd --help
mcd --version
mcd help <command>
```

| Option | Purpose |
| --- | --- |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the CLI version. |

## `inspect`

```bash
mcd inspect <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to inspect. |

Example:

```bash
mcd inspect examples/minimal/minimal.mcd
```

## `add-annotation`

```bash
mcd add-annotation [options] --page <page> <file> <text>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to update. |
| `<text>` | Annotation body text. |

Options:

| Option | Purpose |
| --- | --- |
| `--page <page>` | Required package path/page the annotation targets, for example `content/main.md`. |
| `--line <line>` | Optional 1-based line in the target page. |
| `--id <id>` | Optional stable annotation ID. Generated when omitted. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md --line 18
mcd add-annotation report.mcd "Check this paragraph." --page content/main.md --line 18 --id review-intro
```

## `convert-pdf`

```bash
mcd convert-pdf [options] --output <output> <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | PDF file to convert. |

Options:

| Option | Purpose |
| --- | --- |
| `--output <output>` | Output MCD package path. |
| `--title <title>` | Optional document title. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd convert-pdf source.pdf --output source.mcd
mcd convert-pdf source.pdf --output source.mcd --title "Imported PDF"
```

## `validate`

```bash
mcd validate [options] <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to validate. |

Options:

| Option | Purpose |
| --- | --- |
| `--format <format>` | Output format. Default: `text`. Possible values: `text`, `json`. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd validate report.mcd
mcd validate report.mcd --format json
```

## `extract`

```bash
mcd extract [options] <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to extract from. |

Choose exactly one extraction mode:

| Option | Purpose |
| --- | --- |
| `--json` | Emit canonical JSON. |
| `--markdown` | Emit Markdown. |
| `--tables` | Emit table data. |
| `--schemas` | Emit table schemas, primary keys, foreign keys, and semantic units. |
| `--images` | Emit image metadata. |
| `--annotations` | Emit annotation metadata. |
| `--charts` | Emit chart metadata and source data. |
| `--external-data` | Emit external data references declared by the manifest. |
| `--provenance` | Emit package-level provenance metadata. |
| `--export annotations` | Export annotations by named content type. |

Additional options:

| Option | Purpose |
| --- | --- |
| `--expand-tables` | Expand table directives in Markdown output. Only valid with `--markdown`. |
| `--page <page>` | Filter annotation export by package page/path. |
| `--line <line>` | Filter annotation export by 1-based source line. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd extract report.mcd --json
mcd extract report.mcd --markdown
mcd extract report.mcd --markdown --expand-tables
mcd extract report.mcd --tables
mcd extract report.mcd --schemas
mcd extract report.mcd --images
mcd extract report.mcd --charts
mcd extract report.mcd --external-data
mcd extract report.mcd --provenance
mcd extract report.mcd --annotations
mcd extract report.mcd --annotations --page content/main.md --line 12
```

## `query`

```bash
mcd query [options] <file> <sql>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to query. |
| `<sql>` | Read-only SQL query. Manifest table IDs are available as table names. |

Options:

| Option | Purpose |
| --- | --- |
| `--format <format>` | Output format: `table`, `json`, or `csv`. Defaults to `table`. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd query report.mcd "select count(*) as rows from revenue"
mcd query report.mcd "select quarter, revenue_gbp from revenue order by revenue_gbp desc limit 1"
mcd query report.mcd "select max(revenue_gbp) as max_revenue from revenue" --format json
```

## `tools`

```bash
mcd tools [options] [file]
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `[file]` | Optional package file whose table schemas, keys, relationships, units, external data, and provenance path should be listed. |

Options:

| Option | Purpose |
| --- | --- |
| `--format <format>` | Output format: `text` or `json`. Defaults to `text`. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd tools
mcd tools report.mcd
mcd tools report.mcd --format json
```

## `render`

```bash
mcd render [options] --output <output> <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to render. |

Choose exactly one render target:

| Option | Purpose |
| --- | --- |
| `--html` | Emit standalone HTML or an HTML project directory. |
| `--markdown` | Emit Markdown with package tables embedded as plain Markdown tables. |

Additional options:

| Option | Purpose |
| --- | --- |
| `--output <output>` | Output rendered file path, or a directory for HTML project output. |
| `-h`, `--help` | Print help. |

Examples:

```bash
mcd render report.mcd --html --output report.html
mcd render report.mcd --html --output render/report
mcd render report.mcd --markdown --output report.rendered.md
```

## `pack`

```bash
mcd pack --output <output> <directory>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<directory>` | Unpacked directory. |

Options:

| Option | Purpose |
| --- | --- |
| `--output <output>` | Output package path. |
| `-h`, `--help` | Print help. |

Example:

```bash
mcd pack work/report --output report.mcd
```

## `unpack`

```bash
mcd unpack --output <output> <file>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<file>` | Package file to unpack. |

Options:

| Option | Purpose |
| --- | --- |
| `--output <output>` | Output directory. |
| `-h`, `--help` | Print help. |

Example:

```bash
mcd unpack report.mcd --output work/report
```

## `init`

```bash
mcd init <directory>
```

Arguments:

| Argument | Purpose |
| --- | --- |
| `<directory>` | Directory to initialize. |

Options:

| Option | Purpose |
| --- | --- |
| `-h`, `--help` | Print help. |

Example:

```bash
mcd init work/report
```
