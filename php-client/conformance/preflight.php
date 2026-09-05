<?php

declare(strict_types=1);

/**
 * Does the PHP port's preflight agree with what the program actually did?
 *
 * `Preflight` and `Shortfall` duplicate the program's arithmetic in a third
 * language, and `wasm-client/SPEC.md` §8 says a predicate that disagrees with
 * the program is worse than no predicate at all. The Rust copies are pinned by
 * `pay-on-chain/tests/src/test_preflight.rs`, which drives a live SVM. PHP
 * cannot run LiteSVM, so `test_preflight_fixture.rs` records the account bytes
 * at each interesting instant, the verdict there, and what the program then
 * did; this reads that recording back.
 *
 * Provenance is deliberately different from `vectors.php`'s, and the two must
 * not be merged. `vectors.json` is regenerated from the *published* crate on
 * every run -- an outside opinion about what the right answers are.
 * `preflight-fixture.json` is committed, and comes from the *local* program,
 * because the question here is whether this port agrees with the program you
 * are about to deploy. It moves only when `bin/test-rust` rewrites it, which
 * makes a moved verdict a reviewable diff rather than a silent change.
 *
 * What this does not claim: coverage is exactly what the recorded cases touch.
 * `required_allowance` is not recorded, being the identity function, and a
 * predicate is only pinned at the states the harness happens to reach. Adding
 * a case to `test_preflight_fixture.rs` is what widens it -- one place, both
 * ports.
 *
 *   php php-client/conformance/preflight.php [preflight-fixture.json]
 */

require __DIR__.'/../vendor/autoload.php';

use SolPay\Core\Contract;
use SolPay\Core\Preflight;
use SolPay\Core\Shortfall;
use SolPay\Core\Site;
use SolPay\Core\TokenAccount;

$path = $argv[1] ?? __DIR__.'/preflight-fixture.json';
if (!is_file($path)) {
    // A hard failure, not a skip. The fixture is committed, so a missing one
    // means something is wrong with the checkout rather than with the order
    // somebody ran things in.
    fwrite(STDERR, "no preflight fixture at $path\n");
    fwrite(STDERR, "It is committed; regenerate with bin/test-rust if it is genuinely absent.\n");
    exit(2);
}

$fixture = json_decode((string) file_get_contents($path), true, 512, JSON_THROW_ON_ERROR);
$failed = [];

function check(string $label, bool $ok, string $detail = ''): void
{
    global $failed;
    printf("%s %s%s\n", $ok ? 'ok  ' : 'FAIL', $label, $detail !== '' ? '  '.$detail : '');
    if (!$ok) {
        $failed[] = $label;
    }
}

if ($fixture['cases'] === []) {
    check('preflight fixture has cases', false, 'the file records nothing');
}

foreach ($fixture['cases'] as $case) {
    $name = $case['name'];
    $site = Site::decode((string) hex2bin($case['site_hex']));
    $contract = Contract::decode((string) hex2bin($case['contract_hex']));
    $account = TokenAccount::decode((string) hex2bin($case['token_account_hex']));
    $views = $case['page_views'];

    $wrong = [];
    $expect = static function (string $what, $got, $want) use (&$wrong): void {
        if ($got !== $want) {
            $wrong[] = sprintf('%s %s not %s', $what, var_export($got, true), var_export($want, true));
        }
    };

    $expect('charge', Preflight::charge($site, $views), $case['charge']);
    $expect('will_settle', Preflight::willSettle($contract, $site, $views), $case['will_settle']);
    $expect('views_remaining', Preflight::viewsRemaining($contract, $site), $case['views_remaining']);
    $expect('limit_floor', Preflight::limitFloor($site, $contract), $case['limit_floor']);
    $expect('unpaid', $contract->unpaid(), $case['unpaid']);

    // `can_meter` is a value, not an exception: null when the call would go
    // through, otherwise which constraint stopped it and by how much.
    $blocked = Preflight::canMeter($contract, $site, $views);
    $want = $case['can_meter'];
    if ($want === null && $blocked !== null) {
        $wrong[] = sprintf('can_meter blocked (%s) a call the program accepted', $blocked->kind->name);
    } elseif ($want !== null && $blocked === null) {
        $wrong[] = sprintf('can_meter allowed a call the program refused with %s', $want['kind']);
    } elseif ($want !== null) {
        $expect('can_meter kind', $blocked->kind->name, $want['kind']);
        $expect('can_meter over', $blocked->over, $want['over']);
    }

    // Both shortfalls at once, because SPL reports either as custom error 1
    // and the site's response differs: top up, or re-authorize.
    $d = $case['diagnose'];
    $shortfall = Shortfall::diagnose($account, $d['unpaid']);
    $expect('balance_short', $shortfall->balanceShort, $d['balance_short']);
    $expect('allowance_short', $shortfall->allowanceShort, $d['allowance_short']);
    $expect('delegate_present', $shortfall->delegatePresent, $d['delegate_present']);

    check(
        "$name",
        $wrong === [],
        $wrong === [] ? 'program '.$case['program'].' — '.$case['note'] : implode('; ', $wrong),
    );
}

if ($failed !== []) {
    fwrite(STDERR, "\n".count($failed)." case(s) disagree with the program: ".implode(', ', $failed)."\n");
    exit(1);
}
printf("\n%d case(s) agree with the program\n", count($fixture['cases']));
