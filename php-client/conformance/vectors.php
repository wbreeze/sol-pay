<?php

declare(strict_types=1);

/**
 * Does the PHP port still agree with the published crate?
 *
 * This is drift control in the sense of `wasm-client/SPEC.md` §8.1, and it is
 * the reason this directory exists. `src/Core` is a second implementation of
 * the same encoding in another language, and a divergent port does not fail
 * cleanly: it produces a plausible transaction that does the wrong thing, and
 * then someone signs it. Nothing in the PHPUnit suite catches that, because
 * those tests hardcode the expected values as literals -- deliberately, so a
 * local regression names the assertion that broke. Frozen literals cannot
 * notice the crate moving underneath them. These vectors can, because they are
 * regenerated from the *published* crate on every run.
 *
 * The contrast with `bin/test-node` is worth keeping straight. Node loads the
 * same wasm binary the browser loads and cannot disagree with the crate; that
 * job guards a loading contract. This one guards an agreement between two
 * separate implementations, which is a real thing to lose.
 *
 * Checks the package in `src/Core`, not `pda-spike/php`. The spike answered
 * whether PHP *could* derive addresses; this answers whether the shipped code
 * still does.
 *
 *   php php-client/conformance/vectors.php [vectors.json]
 */

require __DIR__.'/../vendor/autoload.php';

use SolPay\Core\AccountMeta;
use SolPay\Core\Contract;
use SolPay\Core\Instruction;
use SolPay\Core\Ix;
use SolPay\Core\PayError;
use SolPay\Core\Pda;
use SolPay\Core\Program;
use SolPay\Core\Site;
use SolPay\Core\TokenError;
use SolPay\Core\Tx;
use SolPay\Core\TxException;

$path = $argv[1] ?? __DIR__.'/../vectors-gen/vectors.json';
if (!is_file($path)) {
    fwrite(STDERR, "no vector file at $path -- run the generator first (bin/test-php does)\n");
    exit(2);
}

$v = json_decode((string) file_get_contents($path), true, 512, JSON_THROW_ON_ERROR);
$program = new Program($v['program_id']);
$failed = [];

function check(string $label, bool $ok, string $detail = ''): void
{
    global $failed;
    printf("%s %s%s\n", $ok ? 'ok  ' : 'FAIL', $label, $detail !== '' ? '  '.$detail : '');
    if (!$ok) {
        $failed[] = $label;
    }
}

/**
 * Where two messages first disagree, in words. A hex diff of 348 bytes says
 * nothing; "byte 4, in account key 0" says the ordering broke and "byte 0, in
 * the header counts" says the partition did. Offsets come from the vector's
 * own key count, so this is reading, not compiling.
 */
function whereMessagesDiffer(string $got, string $want, int $keyCount): string
{
    if ($got === $want) {
        return intdiv(strlen($got), 2).' bytes';
    }

    $g = (string) hex2bin($got);
    $w = (string) hex2bin($want);
    $n = 0;
    $shared = min(strlen($g), strlen($w));
    while ($n < $shared && $g[$n] === $w[$n]) {
        ++$n;
    }
    if ($n === $shared) {
        return sprintf('same for %d bytes, then length differs: %d vs %d', $n, strlen($g), strlen($w));
    }

    $keysAt = 4;
    $hashAt = $keysAt + 32 * $keyCount;
    $where = match (true) {
        $n < 3 => 'the header counts',
        $n === 3 => 'the account-key count',
        $n < $hashAt => 'account key '.intdiv($n - $keysAt, 32),
        $n < $hashAt + 32 => 'the recent blockhash',
        $n === $hashAt + 32 => 'the instruction count',
        default => 'the compiled instructions',
    };

    return sprintf('byte %d, in %s: %02x not %02x', $n, $where, ord($g[$n]), ord($w[$n]));
}

// Same inputs the generator used: sha256("authority-<i>") / sha256("payer-<i>"),
// as raw bytes, base58 only at the boundary -- which is where this package
// puts every pubkey anyway.
$sites = 0;
$contracts = 0;
foreach ($v['site'] as $row) {
    $i = $row['i'];
    $authority = \SolPay\Core\Base58::encode(hash('sha256', "authority-$i", true));
    $site = Pda::siteAddress($authority, $program->id);
    if ($site['address'] === $row['address'] && $site['bump'] === $row['bump']) {
        ++$sites;
    }

    $payer = \SolPay\Core\Base58::encode(hash('sha256', "payer-$i", true));
    $contract = Pda::contractAddress($site['address'], $payer, $program->id);
    $want = $v['contract'][$i];
    if ($contract['address'] === $want['address'] && $contract['bump'] === $want['bump']) {
        ++$contracts;
    }
}
check('site PDAs', $sites === $v['count'], "$sites/{$v['count']}");
check('contract PDAs', $contracts === $v['count'], "$contracts/{$v['count']}");

// Rebuild the instruction from the accounts the vector records, then compare
// every field of it -- data, order, and both flags. The flags are the half
// `pda-spike/php/verify.php` only recorded rather than checked.
$ms = $v['meter_and_settle'];
$want = $ms['accounts'];
$ix = Ix::meterAndSettle(
    $program,
    $want[0]['pubkey'],
    $want[1]['pubkey'],
    $want[2]['pubkey'],
    $want[4]['pubkey'],
    $want[5]['pubkey'],
    $want[6]['pubkey'],
    $ms['page_views'],
);
check('meter_and_settle data', bin2hex($ix->data) === $ms['data_hex'], bin2hex($ix->data));

$mismatch = null;
foreach ($want as $n => $expected) {
    $got = $ix->accounts[$n] ?? null;
    if ($got === null
        || $got->pubkey !== $expected['pubkey']
        || $got->isSigner !== $expected['is_signer']
        || $got->isWritable !== $expected['is_writable']) {
        $mismatch ??= "account $n";
    }
}
check(
    'meter_and_settle accounts',
    $mismatch === null && count($ix->accounts) === count($want),
    $mismatch ?? count($ix->accounts).' accounts, pubkeys and flags',
);

// The compiled legacy transaction messages, and the wire bytes around them.
//
// This is a byte-for-byte comparison against `solana-message` and
// `solana-transaction`, which is what the vectors were generated for. They
// were generated *before* `SolPay\Tx` was written, on purpose: writing the
// compiler first means hand-verifying wire bytes, which is the trap the
// libsodium PDA shortcut was -- plausible output, no error.
//
// Three cases, reaching branches one case cannot: an empty readonly-signer
// partition, cross-instruction flag merging, and the fee payer prepended
// rather than sorted. `php-client/README.md`, "The order this has to happen
// in", says which is which.
//
// A mismatch on 348 bytes says only "differs", so `whereMessagesDiffer` names
// the section the first differing byte falls in. That is a diagnostic, not a
// check: it reads at fixed offsets and decides nothing.
$cases = $v['transactions'] ?? null;
check('transaction vectors present', is_array($cases) && $cases !== [],
    is_array($cases) ? count($cases).' cases' : 'absent');

foreach ($cases ?? [] as $tx) {
    $name = $tx['name'];

    // Rebuilt from what the generator recorded as compilation's input, so
    // this checks `Tx` against Solana's own crates rather than against
    // another part of this package.
    $instructions = [];
    foreach ($tx['source_instructions'] as $source) {
        $accounts = [];
        foreach ($source['accounts'] as $account) {
            $accounts[] = new AccountMeta($account['pubkey'], $account['is_signer'], $account['is_writable']);
        }
        $instructions[] = new Instruction($source['program_id'], $accounts, (string) hex2bin($source['data_hex']));
    }

    try {
        $message = Tx::compile($instructions, $tx['fee_payer'], $tx['recent_blockhash']);
    } catch (TxException $e) {
        check("$name: message bytes", false, 'Tx::compile refused it -- '.$e->getMessage());
        continue;
    }

    check("$name: message bytes", bin2hex($message) === $tx['message_hex'],
        whereMessagesDiffer(bin2hex($message), $tx['message_hex'], count($tx['account_keys'])));

    $signatures = array_map(static fn (string $hex): string => (string) hex2bin($hex), $tx['signatures_hex']);
    try {
        $wire = Tx::wire($message, $signatures);
    } catch (TxException $e) {
        check("$name: wire bytes", false, 'Tx::wire refused it -- '.$e->getMessage());
        continue;
    }

    check("$name: wire bytes", bin2hex($wire) === $tx['wire_hex'],
        strlen($wire).' bytes = '.count($signatures).' signature(s) + '.strlen($message));
}

$sa = $v['site_account'];
$site = Site::decode((string) hex2bin($sa['data_hex']));
check('Site::decode', $site->authority === $sa['authority']
    && $site->mint === $sa['mint']
    && $site->treasury === $sa['treasury']
    && $site->pagePrice === $sa['page_price']
    && $site->collectionThreshold === $sa['collection_threshold']
    && $site->minLimit === $sa['min_limit']
    && $site->bump === $sa['bump']);

$ca = $v['contract_account'];
$contract = Contract::decode((string) hex2bin($ca['data_hex']));
check('Contract::decode', $contract->site === $ca['site']
    && $contract->payer === $ca['payer']
    && $contract->limit === $ca['limit']
    && $contract->used === $ca['used']
    && $contract->paid === $ca['paid']
    && $contract->bump === $ca['bump']);

// Anchor's offset is applied by the program, so a variant reordered upstream
// silently renumbers every error after it.
$bad = [];
foreach ($v['pay_errors'] as $e) {
    $case = PayError::fromCode($e['code']);
    if ($case === null || $case->name !== $e['name']) {
        $bad[] = $e['name'];
    }
}
check('PayError codes', $bad === [], $bad === [] ? count($v['pay_errors']).' variants' : implode(', ', $bad));

$bad = [];
foreach ($v['token_errors'] as $e) {
    $case = TokenError::fromCode($e['code']);
    if ($case === null || $case->name !== $e['name']) {
        $bad[] = $e['name'];
    }
}
check('TokenError codes', $bad === [], $bad === [] ? count($v['token_errors']).' variants' : implode(', ', $bad));

if ($failed !== []) {
    fwrite(STDERR, "\n".count($failed)." check(s) failed: ".implode(', ', $failed)."\n");
    exit(1);
}
echo "\nall checks passed\n";
