<?php

declare(strict_types=1);

namespace Mcd;

final class Client
{
    private CommandRunner $runner;

    public function __construct(string $binary = 'mcd')
    {
        $this->runner = new CommandRunner($binary);
    }

    public function open(string $path): Document
    {
        return new Document($path, $this->runner);
    }

    public function convertPdf(string $input, string $output, ?string $title = null): Document
    {
        $command = ['convert-pdf', $input, '--output', $output];
        if ($title !== null) {
            $command[] = '--title';
            $command[] = $title;
        }

        $this->runner->run($command);

        return $this->open($output);
    }

    public function init(string $directory): void
    {
        $this->runner->run(['init', $directory]);
    }

    public function pack(string $directory, string $output): Document
    {
        $this->runner->run(['pack', $directory, '--output', $output]);

        return $this->open($output);
    }

    public function unpack(string $path, string $outputDirectory): void
    {
        $this->runner->run(['unpack', $path, '--output', $outputDirectory]);
    }

    public function addAnnotation(
        string $path,
        string $text,
        string $page,
        ?int $line = null,
        ?string $id = null,
    ): string {
        $command = ['add-annotation', $path, $text, '--page', $page];
        if ($line !== null) {
            $command[] = '--line';
            $command[] = (string) $line;
        }
        if ($id !== null) {
            $command[] = '--id';
            $command[] = $id;
        }

        return trim($this->runner->run($command));
    }
}
