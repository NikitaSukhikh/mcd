# MCD Developer Installation Guide

This guide covers installing MCD tooling and bindings from public package
registries.

## CLI

The CLI package is published on crates.io as `mcd-cli` and installs the `mcd`
binary.

```bash
cargo install mcd-cli --version 0.1.0-alpha.2
```

If you do not want to compile locally, download a prebuilt CLI archive from the
GitHub Release:

```text
https://github.com/NikitaSukhikh/mcd/releases/tag/v0.1.0-alpha.2
```

Prebuilt CLI archives are published for:

| Platform | Archive |
| --- | --- |
| Linux x64 | `mcd-cli-linux-x64.tar.gz` |
| Linux arm64 | `mcd-cli-linux-arm64.tar.gz` |
| macOS x64 | `mcd-cli-macos-x64.tar.gz` |
| macOS arm64 | `mcd-cli-macos-arm64.tar.gz` |
| Windows x64 | `mcd-cli-windows-x64.zip` |

Each archive is published with a matching `.sha256` checksum file.

Prebuilt MCP server archives are published for the same platforms:

| Platform | Archive |
| --- | --- |
| Linux x64 | `mcd-mcp-linux-x64.tar.gz` |
| Linux arm64 | `mcd-mcp-linux-arm64.tar.gz` |
| macOS x64 | `mcd-mcp-macos-x64.tar.gz` |
| macOS arm64 | `mcd-mcp-macos-arm64.tar.gz` |
| Windows x64 | `mcd-mcp-windows-x64.zip` |

Verify:

```bash
mcd --help
mcd validate examples/minimal/minimal.mcd
```

If you are working from a source checkout, use:

```bash
cargo run -p mcd-cli -- --help
cargo run -p mcd-cli -- validate examples/minimal/minimal.mcd
```

## Rust

Add the parser/validator crate:

```bash
cargo add mcd-core@0.1.0-alpha.2
```

Add the HTML renderer when your application needs rendered output:

```bash
cargo add mcd-render@0.1.0-alpha.2
```

The raw WebAssembly-facing crate is also published:

```bash
cargo add mcd-wasm@0.1.0-alpha.2
```

For command-line integration from Rust projects, install `mcd-cli` as shown in
the CLI section.

## MCP Server

The official MCP server is published as the Rust `mcd-mcp` binary. It exposes
MCD package validation, inspection, agent context, Markdown extraction, BM25
search, SQL querying, table/chart/image/annotation metadata, rendering,
packing, unpacking, initialization, annotation creation, and PDF conversion
tools to MCP-capable agents.

```bash
cargo install mcd-mcp --version 0.1.0-alpha.2
mcd-mcp --transport stdio
```

From a source checkout:

```bash
cargo run -p mcd-mcp -- --transport stdio
```

Without Cargo, download the matching `mcd-mcp-*` archive from the GitHub
Release, extract the binary locally, and point your MCP client at that local
executable path.

Example local MCP client configuration:

```json
{
  "mcpServers": {
    "mcd": {
      "command": "mcd-mcp",
      "args": ["--transport", "stdio"]
    }
  }
}
```

## Python

The PyPI distribution is `mcdee`; the Python import package is `mcd`.

```bash
pip install mcdee
```

Published releases include prebuilt wheels for common Windows, macOS, and Linux
machines. On those platforms, pip downloads a wheel and does not need Rust,
Maturin, Visual Studio Build Tools, or a local C compiler.

To require a prebuilt wheel and fail instead of compiling from source:

```bash
pip install --only-binary=:all: mcdee
```

Example:

```python
import mcd

doc = mcd.open("report.mcd")
validation = doc.validate()
blocks = doc.blocks()
markdown = doc.markdown(expand_tables=True)
hits = doc.search("thermal_limit_deg_c coolant", limit=5)
```

Optional pandas support:

```bash
pip install "mcdee[pandas]"
```

Optional Python MCP server support for Python-first environments:

```bash
pip install "mcdee[mcp]"
mcd-python-mcp
```

Use `python -m mcd.mcp_server` if you prefer module execution. The default
transport is stdio for local MCP clients.

## TypeScript and JavaScript

The npm package is `@mcd-nix/parser`.

```bash
npm install @mcd-nix/parser
```

Example:

```ts
import { openMcd } from "@mcd-nix/parser";

const bytes = await fetch("report.mcd").then((response) => response.arrayBuffer());
const doc = await openMcd(bytes);

const validation = doc.validate();
const markdown = doc.markdown({ expandTables: true });
```

The npm package embeds the MCD WebAssembly parser and does not require the
`mcd` CLI.

## PHP

The Composer package is `mcd-nix/parser`.

```bash
composer require mcd-nix/parser
```

The PHP package is a wrapper around the `mcd` CLI. Composer installs the PHP
client code, but PHP developers still need the `mcd` binary installed and
available on `PATH`.

Install the CLI with Cargo:

```bash
cargo install mcd-cli --version 0.1.0-alpha.2
```

Or download a prebuilt binary from the GitHub Release:

```text
https://github.com/NikitaSukhikh/mcd/releases/tag/v0.1.0-alpha.2
```

Example:

```php
<?php

use Mcd\Client;

$mcd = new Client();
$doc = $mcd->open('report.mcd');

$validation = $doc->validate();
$blocks = $doc->blocks();
$tables = $doc->tables();
$markdown = $doc->markdown(expandTables: true);
```

If the `mcd` binary is not on `PATH`, pass its full path:

```php
$mcd = new Client('/path/to/mcd');
```

## Package Names

| Ecosystem | Package | Notes |
| --- | --- | --- |
| CLI | `mcd-cli` | Installs the `mcd` binary through Cargo. |
| Rust | `mcd-core` | Parser, validator, and exporter. |
| Rust | `mcd-render` | HTML renderer. |
| Rust | `mcd-wasm` | Raw WebAssembly bindings. |
| Python | `mcdee` | Import as `mcd`. |
| MCP | `mcd-mcp` | Official Rust MCP server for local MCP clients. |
| Python MCP | `mcdee[mcp]` | Python convenience MCP server. |
| npm | `@mcd-nix/parser` | Includes embedded WebAssembly. |
| Composer | `mcd-nix/parser` | PHP wrapper; requires the `mcd` CLI or prebuilt binary. |

## No Compiler Required

These install paths use prebuilt artifacts on supported platforms:

| Tooling | Command or artifact | Local compiler needed |
| --- | --- | --- |
| CLI | GitHub Release archive for your OS/CPU | No |
| Python | `pip install mcdee` | No, when a matching wheel exists |
| Python | `pip install --only-binary=:all: mcdee` | No; fails if no wheel exists |
| TypeScript/JavaScript | `npm install @mcd-nix/parser` | No |
| PHP | `composer require mcd-nix/parser` plus prebuilt `mcd` CLI archive | No |

These install paths may compile locally:

| Tooling | Command | Local compiler needed |
| --- | --- | --- |
| CLI | `cargo install mcd-cli` | Rust toolchain and native linker |
| Rust libraries | `cargo add mcd-core` / `cargo add mcd-render` | Rust toolchain and native linker |
| Python fallback | `pip install mcdee` without a matching wheel | Rust, Maturin, and native build tools |

## Troubleshooting

- If `cargo install mcd-cli` fails on Windows with a missing `kernel32.lib`,
  install or repair the Windows SDK / Visual Studio C++ build tools.
- If `pip install mcdee` tries to build from source on Windows and fails with a
  missing `kernel32.lib`, either install from a prebuilt wheel with
  `pip install --only-binary=:all: mcdee` or run pip from the
  "x64 Native Tools Command Prompt for VS 2022".
- If PHP cannot find `mcd`, run `mcd --help` in the same shell and either fix
  `PATH` or pass the binary path to `new Mcd\Client(...)`.
- If TypeScript bundling fails, make sure your runtime supports ES modules.
