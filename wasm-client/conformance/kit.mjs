// Do this library's instructions still mean to `@solana/kit` what they say?
//
// `src/lib.rs` serialises every instruction as
// `{ programAddress, accounts: [{ address, role }], data }`, and `role` is
// `(is_signer << 1) | is_writable` under a doc comment reading "Matches kit's
// AccountRole". That sentence is the entire agreement, and until this file
// existed nothing anywhere checked it.
//
// **Neither package declares the other**, which is why it needs checking here
// rather than falling out of a version bump. `wasm-client/Cargo.toml` names no
// JavaScript dependency of any kind -- the bundle is wasm-pack output with no
// `import` statement in it -- and a consumer picks kit for itself. So a change
// on either side of the agreement fails *nowhere at install time*. It fails
// when a compiled message reaches a validator, or earlier and worse: a message
// that compiles, signs, and authorises the wrong accounts.
//
// Three claims, in increasing order of how much they are worth:
//
//   1. The shape. Every builder returns the field names kit reads.
//   2. The numbering. kit's own `AccountRole` constants still equal the
//      bit pattern `lib.rs` hardcodes, and kit's own `isSignerRole` /
//      `isWritableRole` still read our `role` the way the program's account
//      list meant it. A silent renumbering upstream would swap signer for
//      writable on every account this library emits, and every shape check
//      ever written would still pass.
//   3. The transaction. kit's legacy compilation of an instruction, given
//      roles in kit's own vocabulary, is compared to what `solana-message`
//      produced for the identical instruction -- over the three cases in
//      `vectors.json` chosen to reach the branches one case cannot (an empty
//      readonly-signer partition, cross-instruction flag merging, and a fee
//      payer prepended rather than sorted). Header, account set, every
//      account's signer and writable bit, the blockhash, and each
//      instruction's program and account list *resolved back to addresses*.
//      This is the claim that matters: header partitioning is where two
//      implementations of a transaction message actually disagree, and a
//      shape check cannot see it.
//
//      **Compared as transactions and not as bytes, because the bytes are
//      legitimately allowed to differ**, and finding that out is what this
//      file was worth on its first real run (2026-09-12, kit 8.3.0). Within a
//      header partition the account order is not canonical: `solana-message`
//      builds its key list from a `BTreeMap<Pubkey, _>`, so it ascends by raw
//      32-byte value, and kit ascends by the base58 *string*. The two agree
//      everywhere those orders agree and disagree on exactly one key in these
//      vectors -- the SPL Token program id, whose raw bytes sort early and
//      whose base58 spelling ("Token...") sorts late.
//
//      Both are valid and a validator accepts either: the runtime checks the
//      partition invariant and resolves instruction accounts by index, and
//      both messages are self-consistent under both. The demonstrator has been
//      sending kit-compiled transactions to devnet since 2026-09-07, which is
//      the empirical half of the same statement.
//
//      The practical consequence, and the reason it is written down here: a
//      message compiled by `SolPay\Tx` on a PHP server and the "same" message
//      compiled by kit in the browser will not be byte-identical, so nothing
//      should ever compare them that way -- not a "confirm what you are
//      signing" check, not a cache key, not a dedupe. Compare the resolved
//      transaction, the way this file does.
//
// Not a loading contract (`conformance/node.mjs`, SPEC §3.1) and not a port
// against the crate (`php-client`, SPEC §8.1). A third kind: this library
// against an outside package neither side pins. SPEC §8.2.
//
//   node wasm-client/conformance/kit.mjs [pkgDir] [vectors.json]
//
// `bin/test-kit` is this with the bundle, the vectors and `npm install`
// arranged in front of it.

import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { argv, exit } from 'node:process';

const here = new URL('.', import.meta.url);
const pkgDir = argv[2] ?? new URL('../pkg', import.meta.url).pathname;
const vectorsPath = argv[3] ?? new URL('../../php-client/vectors-gen/vectors.json', import.meta.url).pathname;

const hex = (u8) => Buffer.from(u8).toString('hex');
const unhex = (s) => new Uint8Array(Buffer.from(s, 'hex'));

const fail = [];
const check = (label, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? '  ' + detail : ''}`);
  if (!ok) fail.push(label);
};

// Bare specifier, resolved from this file's directory: `bin/test-kit` installs
// into conformance/node_modules, so kit is a sibling of this script and not
// something the repository root knows about.
const kit = await import('@solana/kit');

const declared = JSON.parse(await readFile(new URL('package.json', here), 'utf8'));
const range = declared.devDependencies['@solana/kit'];
const installed = JSON.parse(
  await readFile(new URL('node_modules/@solana/kit/package.json', here), 'utf8'),
).version;
console.log(`@solana/kit ${installed}, declared ${range}\n`);

// --- 0. the range reaches the published package -----------------------------
//
// The demonstrator (and any other consumer) reads `peerDependencies` off npm to
// decide whether its own vendored kit is still one this release was asserted
// against. That field is written by `bin/build-rust --client` into gitignored
// output, so nothing in a diff would show it going missing. Here is where it
// shows.
const pkgManifest = JSON.parse(await readFile(`${pkgDir}/package.json`, 'utf8'));
const published = (pkgManifest.peerDependencies ?? {})['@solana/kit'];
check('pkg/package.json publishes the range', published === range, `${published ?? '(absent)'} vs ${range}`);
check(
  'and marks it optional',
  ((pkgManifest.peerDependenciesMeta ?? {})['@solana/kit'] ?? {}).optional === true,
);

// --- the bundle -------------------------------------------------------------
const wasm = await import(pathToFileURL(`${pkgDir}/sol_pay_client.js`).href);
wasm.initSync({ module: await readFile(`${pkgDir}/sol_pay_client_bg.wasm`) });

const v = JSON.parse(await readFile(vectorsPath, 'utf8'));
const pay = new wasm.PayOnChain(v.program_id);

// --- 1. the shape -----------------------------------------------------------
//
// All three transaction builders, not one: each crosses the boundary, and
// `close_and_revoke` pairs its two instructions in the other order.
const ms = v.meter_and_settle;
const a = ms.accounts.map((x) => x.pubkey);
const [site, authority, payer, payerAta, treasury, mint] = [a[0], a[1], a[2], a[4], a[5], a[6]];

const groups = {
  approveAndOpen: pay.approveAndOpen(payerAta, mint, payer, site, 1_000_000n, 6),
  approveAndRenew: pay.approveAndRenew(payerAta, mint, payer, site, 2_000_000n, 6),
  closeAndRevoke: pay.closeAndRevoke(payerAta, payer, site),
  meterAndSettle: [pay.meterAndSettle(site, authority, payer, payerAta, treasury, mint, ms.page_views)],
};

const wellFormed = (ix) =>
  typeof ix.programAddress === 'string' &&
  ix.programAddress.length > 0 &&
  Array.isArray(ix.accounts) &&
  ix.accounts.every((m) => typeof m.address === 'string' && Number.isInteger(m.role) && m.role >= 0 && m.role <= 3) &&
  ix.data instanceof Uint8Array;

for (const [name, ixs] of Object.entries(groups)) {
  check(`${name} returns kit-shaped instructions`, Array.isArray(ixs) && ixs.length > 0 && ixs.every(wellFormed), `${ixs.length} instruction(s)`);
}

// --- 2. the numbering -------------------------------------------------------
//
// `lib.rs` hardcodes the bit pattern rather than importing it from anywhere,
// because it cannot import it from anywhere. This is the assertion that keeps
// that hardcoding honest.
check('AccountRole.READONLY === 0', kit.AccountRole.READONLY === 0, String(kit.AccountRole.READONLY));
check('AccountRole.WRITABLE === 1', kit.AccountRole.WRITABLE === 1, String(kit.AccountRole.WRITABLE));
check('AccountRole.READONLY_SIGNER === 2', kit.AccountRole.READONLY_SIGNER === 2, String(kit.AccountRole.READONLY_SIGNER));
check('AccountRole.WRITABLE_SIGNER === 3', kit.AccountRole.WRITABLE_SIGNER === 3, String(kit.AccountRole.WRITABLE_SIGNER));

// And the round trip through kit's own readers, against the flags the vectors
// carry from the real crate: emitted role in, signer/writable out, matching
// what the program's account list says it should be.
{
  const emitted = groups.meterAndSettle[0].accounts;
  const wrong = ms.accounts
    .map((want, i) => ({ want, got: emitted[i] }))
    .filter(({ want, got }) =>
      got.address !== want.pubkey ||
      kit.isSignerRole(got.role) !== want.is_signer ||
      kit.isWritableRole(got.role) !== want.is_writable);
  check('kit reads every meter_and_settle role as the crate meant it', wrong.length === 0,
    wrong.length ? JSON.stringify(wrong[0]) : `${emitted.length} accounts`);
}

// --- 3. the same transaction ------------------------------------------------
//
// The demonstrator's exact pipeline, and deliberately with plain strings rather
// than `address()`-branded ones: kit's branding is a type, the strings a server
// hands the browser are strings, and this is the call that actually happens.
const compile = (t) => {
  const instructions = t.source_instructions.map((si) => ({
    programAddress: si.program_id,
    accounts: si.accounts.map((m) => ({
      address: m.pubkey,
      role: (m.is_signer ? 2 : 0) | (m.is_writable ? 1 : 0),
    })),
    data: unhex(si.data_hex),
  }));

  const message = kit.pipe(
    kit.createTransactionMessage({ version: 'legacy' }),
    (m) => kit.setTransactionMessageFeePayer(t.fee_payer, m),
    (m) => kit.setTransactionMessageLifetimeUsingBlockhash(
      { blockhash: t.recent_blockhash, lastValidBlockHeight: 0n }, m),
    (m) => kit.appendTransactionMessageInstructions(instructions, m),
  );

  return kit.compileTransaction(message);
};

// A legacy message, taken apart. Deliberately hand-written and not borrowed
// from either side: a decoder from `solana-message` or from kit would make this
// a check of that side against itself.
const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const b58 = (bytes) => {
  let n = 0n;
  for (const b of bytes) n = n * 256n + BigInt(b);
  let out = '';
  while (n > 0n) { out = B58[Number(n % 58n)] + out; n /= 58n; }
  for (const b of bytes) { if (b !== 0) break; out = '1' + out; }
  return out;
};

const decodeLegacy = (bytes) => {
  let i = 0;
  const compactU16 = () => {
    let value = 0, shift = 0, byte;
    do { byte = bytes[i++]; value |= (byte & 0x7f) << shift; shift += 7; } while (byte & 0x80);
    return value;
  };

  const header = { signatures: bytes[i++], readonlySigned: bytes[i++], readonlyUnsigned: bytes[i++] };
  const keys = [];
  for (let n = compactU16(); keys.length < n; ) { keys.push(b58(bytes.slice(i, i + 32))); i += 32; }
  const blockhash = b58(bytes.slice(i, i + 32)); i += 32;

  const instructions = [];
  for (let n = compactU16(); instructions.length < n; ) {
    const program = keys[bytes[i++]];
    const accounts = [];
    for (let a = compactU16(); accounts.length < a; ) accounts.push(keys[bytes[i++]]);
    const dataLen = compactU16();
    const data = Buffer.from(bytes.slice(i, i + dataLen)).toString('hex');
    i += dataLen;
    instructions.push({ program, accounts, data });
  }

  // Position decides the flags, which is the whole of a legacy header: signers
  // come first, and writables come first within each of the two groups.
  const flags = Object.fromEntries(keys.map((key, k) => [key, {
    signer: k < header.signatures,
    writable: k < header.signatures - header.readonlySigned ||
      (k >= header.signatures && k < keys.length - header.readonlyUnsigned),
  }]));

  return { header, keys, blockhash, instructions, flags, trailing: bytes.length - i };
};

const same = (x, y) => JSON.stringify(x) === JSON.stringify(y);

for (const t of v.transactions) {
  const tx = compile(t);
  const rust = decodeLegacy(unhex(t.message_hex));
  const got = decodeLegacy(new Uint8Array(tx.messageBytes));

  check(`${t.name}: header`, same(rust.header, got.header),
    `${JSON.stringify(got.header)}${same(rust.header, got.header) ? '' : ' vs ' + JSON.stringify(rust.header)}`);

  check(`${t.name}: the same accounts`, same([...rust.keys].sort(), [...got.keys].sort()),
    `${got.keys.length} keys`);

  // Per key rather than per position, because the positions are allowed to
  // differ and the flags are not. This is where a renumbered AccountRole would
  // surface even if claim 2 somehow passed.
  const wrongFlags = rust.keys.filter((k) => !same(rust.flags[k], got.flags[k]));
  check(`${t.name}: signer and writable per account`, wrongFlags.length === 0,
    wrongFlags.length ? wrongFlags.map((k) => `${k.slice(0, 6)} ${JSON.stringify(rust.flags[k])} vs ${JSON.stringify(got.flags[k])}`).join('; ') : 'all match');

  check(`${t.name}: blockhash`, rust.blockhash === got.blockhash, got.blockhash);

  // The payload: program and account list resolved back through each message's
  // own key table, so an ordering difference cancels out and a *wrong index*
  // does not.
  check(`${t.name}: instructions resolve identically`, same(rust.instructions, got.instructions),
    same(rust.instructions, got.instructions) ? `${got.instructions.length} instruction(s)` : JSON.stringify(got.instructions).slice(0, 300));

  check(`${t.name}: nothing trailing`, rust.trailing === 0 && got.trailing === 0);

  // The framing the browser path actually produces. kit has no signatures to
  // put in, so the slots are zeros; the claim is the count and the offset.
  const sigs = t.header.num_required_signatures;
  const wire = new Uint8Array(kit.getTransactionEncoder().encode(tx));
  const offset = 1 + 64 * sigs; // compact-u16 is one byte below 128
  check(`${t.name}: wire framing`,
    sigs < 128 && wire[0] === sigs && wire.length === offset + tx.messageBytes.length &&
      same([...wire.slice(offset)], [...new Uint8Array(tx.messageBytes)]),
    `${sigs} signature slot(s), ${wire.length} bytes`);

  // Not a check. Recorded because the day it stops being true is worth
  // noticing, and because someone will otherwise re-derive the finding above
  // from a confusing diff.
  const identical = hex(new Uint8Array(tx.messageBytes)) === t.message_hex;
  console.log(`     note: ${t.name} encodes ${identical ? 'identically to' : 'differently from'} solana-message` +
    (identical ? '' : ' (intra-partition account order; see the header of this file)'));
}

if (fail.length) {
  console.error(`\n${fail.length} check(s) failed: ${fail.join(', ')}`);
  exit(1);
}
console.log('\nall checks passed');
