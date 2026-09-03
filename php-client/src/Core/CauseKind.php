<?php

declare(strict_types=1);

namespace SolPay\Core;

enum CauseKind
{
    case Program;
    case Token;

    /**
     * Deliberate. The runtime can surface errors from programs neither this
     * package nor the integrator anticipated, and mapping those onto a
     * known enum would be a lie.
     */
    case Unknown;
}
