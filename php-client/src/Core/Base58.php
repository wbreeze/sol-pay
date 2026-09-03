<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Bitcoin-alphabet base58, byte-array long division. No bignum extension.
 * The boundary encoding this package uses for every pubkey, matching
 * wasm-client's JS boundary (see wasm-client/README.md, "addresses cross
 * its boundary as base58 strings, never as a Pubkey object").
 */
final class Base58
{
    private const A = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

    public static function decode(string $s): string
    {
        $bytes = [];
        for ($i = 0, $n = strlen($s); $i < $n; $i++) {
            $v = strpos(self::A, $s[$i]);
            if ($v === false) {
                throw new \InvalidArgumentException("bad base58 char {$s[$i]}");
            }
            $carry = $v;
            for ($j = count($bytes) - 1; $j >= 0; $j--) {
                $carry += 58 * $bytes[$j];
                $bytes[$j] = $carry & 0xFF;
                $carry >>= 8;
            }
            while ($carry > 0) {
                array_unshift($bytes, $carry & 0xFF);
                $carry >>= 8;
            }
        }
        // every leading '1' encodes one leading zero byte
        for ($i = 0; $i < strlen($s) && $s[$i] === '1'; $i++) {
            array_unshift($bytes, 0);
        }
        return $bytes === [] ? '' : pack('C*', ...$bytes);
    }

    public static function encode(string $bin): string
    {
        $bytes = array_values(unpack('C*', $bin));
        $out = '';
        $zeros = 0;
        while ($zeros < count($bytes) && $bytes[$zeros] === 0) {
            $zeros++;
        }
        $start = $zeros;
        while ($start < count($bytes)) {
            $carry = 0;
            for ($i = $start; $i < count($bytes); $i++) {
                $cur = ($carry << 8) + $bytes[$i];
                $bytes[$i] = intdiv($cur, 58);
                $carry = $cur % 58;
            }
            $out = self::A[$carry] . $out;
            while ($start < count($bytes) && $bytes[$start] === 0) {
                $start++;
            }
        }
        return str_repeat('1', $zeros) . $out;
    }
}
