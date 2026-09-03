<?php

declare(strict_types=1);

namespace SolPay\Core;

/** Why a metering call would be refused. Mirrors wasm-client/src/core/preflight.rs's Blocked. */
enum BlockedKind
{
    /**
     * The charge would carry `used` past the authorized limit. The program
     * refuses the whole call rather than metering part of it, so the site
     * must renew or stop, not meter fewer views and hope.
     */
    case LimitReached;

    /**
     * The charge itself does not fit in this package's arithmetic. Only
     * reachable with an absurd page count; the program raises MathOverflow
     * for the same case.
     */
    case Overflow;
}
