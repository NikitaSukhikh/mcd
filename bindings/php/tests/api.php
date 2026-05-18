<?php

declare(strict_types=1);

require __DIR__ . '/../src/Exception/McdException.php';
require __DIR__ . '/../src/Exception/CommandFailedException.php';
require __DIR__ . '/../src/Exception/JsonDecodeException.php';
require __DIR__ . '/../src/Exception/NotFoundException.php';
require __DIR__ . '/../src/CommandRunner.php';
require __DIR__ . '/../src/Document.php';
require __DIR__ . '/../src/Client.php';

use Mcd\Client;

$binary = getenv('MCD_BINARY') ?: 'mcd';
$fixture = realpath(__DIR__ . '/../../../tests/fixtures/conformance/valid-minimal.mcd');

if ($fixture === false) {
    fwrite(STDERR, "Fixture not found.\n");
    exit(1);
}

$doc = (new Client($binary))->open($fixture);

expect($doc->path() === $fixture, 'document path matches fixture');
expect($doc->validate()['valid'] === true, 'fixture validates');
expect($doc->blocks()[0]['type'] === 'heading', 'blocks are extracted');
expect(str_contains($doc->markdown(expandTables: true), '# Minimal'), 'markdown is extracted');
expect($doc->annotations() === [], 'annotations are extracted');

echo "ok\n";

function expect(bool $condition, string $message): void
{
    if (!$condition) {
        fwrite(STDERR, "Failed assertion: {$message}\n");
        exit(1);
    }
}
