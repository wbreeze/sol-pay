<?php

declare(strict_types=1);

namespace SolPay\Core;

/** Decimal strings, not floats. Mirrors wasm-client/src/core/units.rs's UnitsError. */
enum UnitsErrorKind
{
    case Empty;
    case NotANumber;

    /**
     * More digits after the point than the mint has decimals, so the value
     * cannot be represented without discarding some of what was asked for.
     */
    case TooPrecise;

    /** The amount does not fit in this package's integer range. */
    case Overflow;
}
