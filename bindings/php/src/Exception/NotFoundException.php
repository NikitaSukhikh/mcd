<?php

declare(strict_types=1);

namespace Mcd\Exception;

use OutOfBoundsException;

final class NotFoundException extends OutOfBoundsException implements McdException
{
}
