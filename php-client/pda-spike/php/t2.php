<?php
require __DIR__ . '/Pda.php';

$PROGRAM = Base58::decode('F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL');

echo "=== 1. Positive control: real Ed25519 public keys ===\n";
$bad = 0;
for ($i = 0; $i < 200; $i++) {
    $pk = sodium_crypto_sign_publickey(sodium_crypto_sign_keypair());
    $mine = Ed25519::isOnCurve($pk);
    $sod  = Pda::sodiumIsValidPoint($pk);
    if (!$mine || !$sod) { $bad++; }
}
printf("strict predicate in use: %s\n", Pda::strictPredicateName());
printf("200 generated public keys: %s\n", $bad === 0
    ? "all on-curve by both predicates (as they must be)"
    : "$bad DISAGREEMENTS -- implementation is wrong");

echo "\n=== 2. Random 32-byte values: is_valid_point vs is_on_curve ===\n";
$N = 4000;
$onCurve = 0; $valid = 0; $violations = 0; $disagree = 0;
for ($i = 0; $i < $N; $i++) {
    $b = hash('sha256', "sample-$i", true);
    $mine = Ed25519::isOnCurve($b);
    $sod  = Pda::sodiumIsValidPoint($b);
    if ($mine) { $onCurve++; }
    if ($sod)  { $valid++; }
    // libsodium's predicate is strictly stronger, so valid => on-curve must hold.
    if ($sod && !$mine) { $violations++; }
    if ($sod !== $mine) { $disagree++; }
}
printf("samples                              %d\n", $N);
printf("on curve (Solana is_on_curve)        %d  (%.1f%%)\n", $onCurve, 100*$onCurve/$N);
printf("valid point (libsodium)              %d  (%.1f%%)\n", $valid, 100*$valid/$N);
printf("disagreements                        %d  (%.1f%%)\n", $disagree, 100*$disagree/$N);
printf("invariant violations (valid && !on-curve)  %d   <- must be 0\n", $violations);

echo "\n=== 3. PDA derivation: does the wrong predicate give wrong addresses? ===\n";
$M = 400;
$siteDiff = 0; $contractDiff = 0; $samples = [];
for ($i = 0; $i < $M; $i++) {
    $authority = hash('sha256', "authority-$i", true);
    [$sA, $bA] = Pda::findProgramAddress(['site', $authority], $PROGRAM);
    [$sB, $bB] = Pda::findProgramAddress(['site', $authority], $PROGRAM, [Pda::class, 'sodiumIsValidPoint']);
    if ($sA !== $sB) {
        $siteDiff++;
        if (count($samples) < 3) {
            $samples[] = sprintf("  authority %s\n    correct  %s (bump %d)\n    sodium   %s (bump %d)",
                substr(Base58::encode($authority), 0, 12) . '...',
                Base58::encode($sA), $bA, Base58::encode($sB), $bB);
        }
    }
    $payer = hash('sha256', "payer-$i", true);
    [$cA, ] = Pda::findProgramAddress(['contract', $sA, $payer], $PROGRAM);
    [$cB, ] = Pda::findProgramAddress(['contract', $sA, $payer], $PROGRAM, [Pda::class, 'sodiumIsValidPoint']);
    if ($cA !== $cB) { $contractDiff++; }
}
printf("site PDAs differing:      %d / %d  (%.1f%%)\n", $siteDiff, $M, 100*$siteDiff/$M);
printf("contract PDAs differing:  %d / %d  (%.1f%%)\n", $contractDiff, $M, 100*$contractDiff/$M);
if ($samples) { echo "\nexamples:\n" . implode("\n", $samples) . "\n"; }
