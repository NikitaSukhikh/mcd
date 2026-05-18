<?php

declare(strict_types=1);

namespace Mcd;

use Mcd\Exception\CommandFailedException;
use Mcd\Exception\NotFoundException;

final class Document
{
    private ?array $json = null;

    public function __construct(
        private readonly string $path,
        private readonly CommandRunner $runner,
    ) {
    }

    public function path(): string
    {
        return $this->path;
    }

    /**
     * @return array{valid: bool, diagnostics: list<array<string, mixed>>}
     */
    public function validate(): array
    {
        try {
            /** @var array{valid: bool, diagnostics: list<array<string, mixed>>} $result */
            $result = $this->runner->runJson(['validate', $this->path, '--format', 'json']);

            return $result;
        } catch (CommandFailedException $exception) {
            $decoded = json_decode($exception->stdout(), true);
            if (is_array($decoded) && array_key_exists('valid', $decoded)) {
                /** @var array{valid: bool, diagnostics: list<array<string, mixed>>} $decoded */
                return $decoded;
            }

            throw $exception;
        }
    }

    /**
     * @return array<string, mixed>
     */
    public function json(): array
    {
        return $this->json ??= $this->runner->runJson(['extract', $this->path, '--json']);
    }

    /**
     * @return list<array<string, mixed>>
     */
    public function blocks(): array
    {
        $json = $this->json();
        $blocks = $json['document']['blocks'] ?? [];

        return is_array($blocks) ? $blocks : [];
    }

    public function markdown(bool $expandTables = false): string
    {
        $command = ['extract', $this->path, '--markdown'];
        if ($expandTables) {
            $command[] = '--expand-tables';
        }

        return rtrim($this->runner->run($command), "\r\n");
    }

    /**
     * @return list<array<string, mixed>>
     */
    public function tables(): array
    {
        $result = $this->runner->runJson(['extract', $this->path, '--tables']);
        $tables = $result['tables'] ?? [];

        return is_array($tables) ? $tables : [];
    }

    /**
     * @return array<string, mixed>
     */
    public function table(string $id): array
    {
        return $this->findById($this->tables(), $id, 'table');
    }

    /**
     * @return list<array<string, mixed>>
     */
    public function images(): array
    {
        $result = $this->runner->runJson(['extract', $this->path, '--images']);
        $images = $result['images'] ?? [];

        return is_array($images) ? $images : [];
    }

    /**
     * @return array<string, mixed>
     */
    public function image(string $id): array
    {
        foreach ($this->images() as $image) {
            if (
                ($image['id'] ?? null) === $id
                || ($image['asset'] ?? null) === $id
                || (
                    isset($image['asset'])
                    && str_starts_with((string) $image['asset'], 'assets/')
                    && substr((string) $image['asset'], 7) === $id
                )
            ) {
                return $image;
            }
        }

        throw new NotFoundException("Unknown image '{$id}'.");
    }

    /**
     * @return list<array<string, mixed>>
     */
    public function charts(): array
    {
        $result = $this->runner->runJson(['extract', $this->path, '--charts']);
        $charts = $result['charts'] ?? [];

        return is_array($charts) ? $charts : [];
    }

    /**
     * @return array<string, mixed>
     */
    public function chart(string $id): array
    {
        foreach ($this->charts() as $chart) {
            if (
                ($chart['blockId'] ?? null) === $id
                || ($chart['viewId'] ?? null) === $id
                || ($chart['placementRef'] ?? null) === $id
            ) {
                return $chart;
            }
        }

        throw new NotFoundException("Unknown chart '{$id}'.");
    }

    /**
     * @return list<array<string, mixed>>
     */
    public function annotations(?string $page = null, ?int $line = null): array
    {
        $command = ['extract', $this->path, '--annotations'];
        if ($page !== null) {
            $command[] = '--page';
            $command[] = $page;
        }
        if ($line !== null) {
            $command[] = '--line';
            $command[] = (string) $line;
        }

        $result = $this->runner->runJson($command);
        $annotations = $result['annotations'] ?? [];

        return is_array($annotations) ? $annotations : [];
    }

    /**
     * @return array<string, mixed>
     */
    public function annotation(string $id): array
    {
        return $this->findById($this->annotations(), $id, 'annotation');
    }

    /**
     * @param list<array<string, mixed>> $items
     * @return array<string, mixed>
     */
    private function findById(array $items, string $id, string $type): array
    {
        foreach ($items as $item) {
            if (($item['id'] ?? null) === $id) {
                return $item;
            }
        }

        throw new NotFoundException("Unknown {$type} '{$id}'.");
    }
}
