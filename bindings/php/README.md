# mcd PHP wrapper

PHP wrapper for Markdown CSV Document packages.

This package delegates parsing and conversion work to the `mcd` CLI. Composer
installs the PHP client code, but PHP developers still need the `mcd` binary
installed and available on `PATH`.

Install the CLI with Cargo:

```bash
cargo install mcd-cli --version 0.1.0-alpha.1
```

Or download a prebuilt binary from:

```text
https://github.com/NikitaSukhikh/mcd/releases/tag/v0.1.0-alpha.1
```

To install from a checkout of this package:

```bash
composer install
```

From Packagist, use:

```bash
composer require mcd-nix/parser
```

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

If the `mcd` binary is not on `PATH`, pass the binary path:

```php
$mcd = new Client('/path/to/mcd');
```
