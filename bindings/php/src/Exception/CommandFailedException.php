<?php

declare(strict_types=1);

namespace Mcd\Exception;

use RuntimeException;

final class CommandFailedException extends RuntimeException implements McdException
{
    /**
     * @param list<string> $command
     */
    public function __construct(
        private readonly array $command,
        private readonly int $exitCode,
        private readonly string $stdout,
        private readonly string $stderr,
    ) {
        $message = trim($stderr) !== '' ? trim($stderr) : 'mcd command failed.';
        parent::__construct($message, $exitCode);
    }

    /**
     * @return list<string>
     */
    public function command(): array
    {
        return $this->command;
    }

    public function exitCode(): int
    {
        return $this->exitCode;
    }

    public function stdout(): string
    {
        return $this->stdout;
    }

    public function stderr(): string
    {
        return $this->stderr;
    }
}
