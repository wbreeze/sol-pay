<?php
require __DIR__ . '/Curve25519.php';
$fail = 0;
function ok(string $name, bool $cond) { global $fail; if (!$cond) { $fail++; echo "FAIL  $name\n"; } else { echo "ok    $name\n"; } }

// p reduces to zero
ok('canonical(p) == 0', Fe::isZero(Fe::p()));

// inversion round-trips
$a = Fe::fromBytes(hash('sha256', 'inversion probe', true));
$ai = Ed25519::invert($a);
ok('a * a^-1 == 1', Fe::equals(Fe::mul($a, $ai), Fe::fromInt(1)));

// the curve constant d, computed from -121665/121666, against the published value
$D_EXPECTED = '52036CEE2B6FFE738CC740797779E89800700A4D4141D8AB75EB4DCA135978A3';
$d = Ed25519::d();
ok('d == published EDWARDS_D', Fe::toHex($d) === $D_EXPECTED);
if (Fe::toHex($d) !== $D_EXPECTED) { echo "      got " . Fe::toHex($d) . "\n"; }

// small-value arithmetic against plain ints
ok('7 * 9 == 63', Fe::equals(Fe::mul(Fe::fromInt(7), Fe::fromInt(9)), Fe::fromInt(63)));
ok('(p-1) + 2 == 1', Fe::equals(Fe::add(Fe::sub(Fe::zero(), Fe::fromInt(1)), Fe::fromInt(2)), Fe::fromInt(1)));

// 4 is a square, 2 is not (2 is a non-residue mod 2^255-19)
$chi4 = Fe::powBytes(Fe::fromInt(4), pack('H*', '3ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff6'));
echo "note  chi(4) = " . substr(Fe::toHex($chi4), -4) . "\n";

exit($fail === 0 ? 0 : 1);
