# mcd PHP wrapper

PHP wrapper for Markdown CSV Document packages.

This package delegates parsing and conversion work to the `mcd` CLI, so install
the CLI first:

```bash
cargo install --path crates/mcd-cli
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
