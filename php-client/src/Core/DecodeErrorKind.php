<?php

declare(strict_types=1);

namespace SolPay\Core;

/** Why decoding an account's bytes failed. Mirrors wasm-client/src/core/state.rs's DecodeError. */
enum DecodeErrorKind
{
    /** The account belongs to something else, or to a later layout. */
    case WrongDiscriminator;
    case WrongLength;
}
