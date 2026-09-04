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

use SolPay\Core\Contract;
use SolPay\Core\Ix;
use SolPay\Core\PayError;
use SolPay\Core\Pda;
use SolPay\Core\Program;
use SolPay\Core\Site;
use SolPay\Core\TokenError;

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
// `SolPay\Tx` does not exist yet, and that is the point: these vectors were
// generated first so the encoder has something to be written against rather
// than hand-verified after the fact. Until it exists there is nothing here to
// compare *against*, so what follows checks that each vector is the thing the
// encoder will need -- present, internally consistent, and agreeing with the
// instructions that went into it, which a different crate produced.
//
// It reads each message at fixed offsets and reads the partition the header
// counts imply. It orders nothing and derives no count. **It must not grow
// into a second implementation of what `Tx::compile` will do** -- when `Tx`
// lands, a byte-for-byte comparison replaces these shape checks rather than
// joining them.
$cases = $v['transactions'] ?? null;
check('transaction vectors present', is_array($cases) && $cases !== [],
    is_array($cases) ? count($cases).' cases' : 'absent');

foreach ($cases ?? [] as $tx) {
    $name = $tx['name'];
    $keys = $tx['account_keys'];
    $hdr = $tx['header'];
    $msg = (string) hex2bin($tx['message_hex']);
    $src = $tx['source_instructions'];

    // What compilation had to work out, worked out from the same inputs it
    // got: each key's flags OR'd across every instruction, the invoked
    // program ids joining as readonly non-signers unless an instruction
    // already named them, and the fee payer forced to a writable signer
    // whatever the instructions said. Reproducing this map is not compiling a
    // message -- there is no ordering and no encoding in it.
    $merged = [];
    foreach ($src as $i) {
        foreach ($i['accounts'] as $a) {
            $was = $merged[$a['pubkey']] ?? ['is_signer' => false, 'is_writable' => false];
            $merged[$a['pubkey']] = [
                'is_signer' => $was['is_signer'] || $a['is_signer'],
                'is_writable' => $was['is_writable'] || $a['is_writable'],
            ];
        }
    }
    foreach ($src as $i) {
        $merged[$i['program_id']] ??= ['is_signer' => false, 'is_writable' => false];
    }
    $merged[$tx['fee_payer']] = ['is_signer' => true, 'is_writable' => true];

    check("$name: fee payer leads account_keys", ($keys[0] ?? null) === $tx['fee_payer']);

    // Exactly the merged set: deduplicated, nothing missing, nothing extra.
    $extra = array_values(array_diff($keys, array_keys($merged)));
    $missing = array_values(array_diff(array_keys($merged), $keys));
    check("$name: account_keys are the merged set",
        count(array_unique($keys)) === count($keys) && $extra === [] && $missing === [],
        $extra === [] && $missing === []
            ? count($keys).' keys'
            : 'extra: '.implode(',', $extra).' missing: '.implode(',', $missing));

    $signers = array_filter($merged, static fn (array $m): bool => $m['is_signer']);
    check("$name: header signature count", $hdr['num_required_signatures'] === count($signers),
        $hdr['num_required_signatures'].' required, '.count($signers).' signers');

    // The header does not merely count -- it *partitions* the key list.
    // Signers first, writable before readonly within each half, so the three
    // counts alone determine which keys the runtime will let the program
    // write. Hold that partition against the merged flags.
    $req = $hdr['num_required_signatures'];
    $writableSigners = $req - $hdr['num_readonly_signed_accounts'];
    $writableOthers = count($keys) - $req - $hdr['num_readonly_unsigned_accounts'];
    $bad = [];
    foreach ($keys as $n => $key) {
        $isSigner = $n < $req;
        $isWritable = $isSigner ? $n < $writableSigners : $n < $req + $writableOthers;
        $want = $merged[$key] ?? ['is_signer' => false, 'is_writable' => false];
        if ($isSigner !== $want['is_signer'] || $isWritable !== $want['is_writable']) {
            $bad[] = "key $n";
        }
    }
    check("$name: header partitions the keys", $bad === [],
        $bad === [] ? "$writableSigners writable signer(s), $writableOthers writable non-signer(s)" : implode(', ', $bad));

    // Inside each partition the keys ascend by raw pubkey bytes -- NOT by the
    // order the instructions named them. An encoder that keeps instruction
    // order within a partition builds a different message that still looks
    // right, which is the single most likely way to get this wrong.
    //
    // The fee payer is the exception, and it is a rule rather than an
    // accident: compilation pulls it out and puts it first instead of sorting
    // it into place. The "two-instructions" case has a second writable signer
    // that sorts *before* the fee payer, so an encoder that sorts them all
    // together passes the other cases and fails that one.
    $bounds = [
        'writable signers' => [0, $writableSigners],
        'readonly signers' => [$writableSigners, $req],
        'writable non-signers' => [$req, $req + $writableOthers],
        'readonly non-signers' => [$req + $writableOthers, count($keys)],
    ];
    $bad = [];
    foreach ($bounds as $label => [$from, $to]) {
        $slice = array_slice($keys, $from, $to - $from);
        if ($from === 0) {
            array_shift($slice);   // the fee payer, prepended rather than sorted
        }
        $raw = array_map(static fn (string $k): string => \SolPay\Core\Base58::decode($k), $slice);
        $sorted = $raw;
        sort($sorted, SORT_STRING);
        if ($raw !== $sorted) {
            $bad[] = $label;
        }
    }
    check("$name: partitions sorted by pubkey", $bad === [],
        $bad === [] ? 'fee payer first, the rest ascending' : implode(', ', $bad));

    // The header is the first three bytes, and the compact-u16 account-key
    // count is the fourth -- single-byte while the count is under 128, which
    // it is here and which `Tx::compile` must not assume in general.
    $wantHead = sprintf('%02x%02x%02x%02x',
        $hdr['num_required_signatures'],
        $hdr['num_readonly_signed_accounts'],
        $hdr['num_readonly_unsigned_accounts'],
        count($keys));
    check("$name: message header and key count", str_starts_with($tx['message_hex'], $wantHead),
        substr($tx['message_hex'], 0, 8).' vs '.$wantHead);

    // The keys, the blockhash, then the instruction count, at the offsets
    // those counts imply.
    $bad = [];
    foreach ($keys as $n => $key) {
        if (substr($msg, 4 + 32 * $n, 32) !== \SolPay\Core\Base58::decode($key)) {
            $bad[] = "key $n";
        }
    }
    check("$name: message account_keys", $bad === [], $bad === [] ? count($keys).' in order' : implode(', ', $bad));

    $hashAt = 4 + 32 * count($keys);
    check("$name: message recent_blockhash",
        substr($msg, $hashAt, 32) === \SolPay\Core\Base58::decode($tx['recent_blockhash']));

    check("$name: message instruction count",
        substr($msg, $hashAt + 32, 1) === chr(count($src)) && count($tx['instructions']) === count($src),
        count($src).' instruction(s)');

    // Compilation moves accounts into indexes; it must not touch the payload,
    // reorder an instruction's accounts, or lose which program is called.
    $bad = [];
    foreach ($tx['instructions'] as $n => $ci) {
        $want = $src[$n] ?? null;
        if ($want === null) {
            $bad[] = "instruction $n unmatched";
            continue;
        }
        if (($keys[$ci['program_id_index']] ?? null) !== $want['program_id']) {
            $bad[] = "instruction $n program id";
        }
        $order = [];
        foreach ($ci['account_indexes'] as $idx) {
            $order[] = $keys[$idx] ?? null;
        }
        if ($order !== array_column($want['accounts'], 'pubkey')) {
            $bad[] = "instruction $n account order";
        }
        if ($ci['data_hex'] !== $want['data_hex']) {
            $bad[] = "instruction $n data";
        }
    }
    check("$name: compiled instructions", $bad === [],
        $bad === [] ? count($tx['instructions']).' matching source' : implode(', ', $bad));

    // Wire framing: compact-u16 signature count, the signatures, then the
    // message. One signature per required signature, and the count is a
    // single byte at these sizes.
    $sigs = $tx['signatures_hex'];
    check("$name: signature count", count($sigs) === $req, count($sigs).' of '.$req);
    $lengths = array_unique(array_map('strlen', $sigs));
    check("$name: signature lengths", $lengths === [128] || $lengths === [], implode(',', $lengths));
    check("$name: wire framing",
        $tx['wire_hex'] === sprintf('%02x', count($sigs)).implode('', $sigs).$tx['message_hex']);
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
