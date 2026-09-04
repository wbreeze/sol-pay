<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * What can go wrong compiling a message or framing a transaction. Every case
 * is a caller mistake caught before bytes are produced; there is no partial
 * output and nothing here is recoverable by retrying.
 */
enum TxErrorKind
{
    /** An address was not 32 bytes once decoded from base58. */
    case BadAddress;

    /**
     * More than 255 distinct accounts across the instructions. Compiled
     * instructions index the key list with single bytes, so a 256th key
     * cannot be referred to at all.
     */
    case TooManyAccounts;

    /** More signatures supplied than the message's header requires, or fewer. */
    case SignatureCount;

    /** A signature was not 64 bytes. */
    case BadSignature;

    /** The message is too short to carry the header its own counts describe. */
    case MalformedMessage;
}
