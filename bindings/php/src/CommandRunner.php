<?php

declare(strict_types=1);

namespace Mcd;

use Mcd\Exception\CommandFailedException;
use Mcd\Exception\JsonDecodeException;

final class CommandRunner
{
    public function __construct(private readonly string $binary)
    {
    }

    /**
     * @param list<string> $arguments
     */
    public function run(array $arguments): string
    {
        $command = array_merge([$this->binary], $arguments);
        $descriptors = [
            0 => ['pipe', 'r'],
            1 => ['pipe', 'w'],
            2 => ['pipe', 'w'],
        ];

        $process = proc_open($command, $descriptors, $pipes);
        if (!is_resource($process)) {
            throw new CommandFailedException($command, -1, '', 'Unable to start mcd process.');
        }

        fclose($pipes[0]);
        $stdout = (string) stream_get_contents($pipes[1]);
        $stderr = (string) stream_get_contents($pipes[2]);
        fclose($pipes[1]);
        fclose($pipes[2]);

        $exitCode = proc_close($process);
        if ($exitCode !== 0) {
            throw new CommandFailedException($command, $exitCode, $stdout, $stderr);
        }

        return $stdout;
    }

    /**
     * @param list<string> $arguments
     * @return array<string, mixed>
     */
    public function runJson(array $arguments): array
    {
        $output = $this->run($arguments);
        $decoded = json_decode($output, true);
        if (!is_array($decoded)) {
            throw new JsonDecodeException('mcd returned invalid JSON: ' . json_last_error_msg());
        }

        return $decoded;
    }
}
