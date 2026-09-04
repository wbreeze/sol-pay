// Does the browser bundle still load and compute correctly under Node?
//
// `wasm-pack --target web` output is not a Node package: its default `init()`
// resolves the .wasm relative to `import.meta.url` and fetches it, and Node's
// fetch does not do `file:` URLs. Passing the bytes instead works, needs no
// bundler, and is the whole of what a Node server has to do differently. This
// checks that it stays true, because it is a property of wasm-pack's generated
// glue rather than of anything in this repository -- a wasm-pack release that
// made fetch unconditional would break every Node integrator silently, and
// nothing else here would notice.
//
// It is *not* drift control in the sense of SPEC.md §8.1. Node runs the same
// wasm binary the browser runs, so it cannot disagree with the crate the way a
// re-implementation can. The vectors are here because computing the right
// answers is the cheapest proof that the module actually initialised, rather
// than that `import` returned an object.
//
//   node wasm-client/conformance/node.mjs [pkgDir] [vectors.json]

import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { argv, exit } from 'node:process';

const pkgDir = argv[2] ?? new URL('../pkg', import.meta.url).pathname;
const vectorsPath = argv[3] ?? new URL('../../php-client/pda-spike/php/vectors.json', import.meta.url).pathname;

const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const base58 = (bytes) => {
  let n = 0n;
  for (const b of bytes) n = n * 256n + BigInt(b);
  let out = '';
  while (n > 0n) { out = B58[Number(n % 58n)] + out; n /= 58n; }
  for (const b of bytes) { if (b !== 0) break; out = '1' + out; }
  return out;
};
const sha256 = (s) => new Uint8Array(createHash('sha256').update(s).digest());
const hex = (u8) => Buffer.from(u8).toString('hex');

const fail = [];
const check = (label, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${detail ? '  ' + detail : ''}`);
  if (!ok) fail.push(label);
};

// The point of the exercise: no fetch, no bundler, bytes handed in.
const wasm = await import(pathToFileURL(`${pkgDir}/sol_pay_client.js`).href);
wasm.initSync({ module: await readFile(`${pkgDir}/sol_pay_client_bg.wasm`) });
check('initSync with bytes, no fetch, no bundler', true);

const v = JSON.parse(await readFile(vectorsPath, 'utf8'));
const pay = new wasm.PayOnChain(v.program_id);

// Same inputs the generator used: sha256("authority-<i>") / sha256("payer-<i>").
let sites = 0, contracts = 0;
for (let i = 0; i < v.count; i++) {
  const site = pay.deriveSiteAddress(base58(sha256(`authority-${i}`)));
  if (site === v.site[i].address) sites++;
  if (pay.deriveContractAddress(site, base58(sha256(`payer-${i}`))) === v.contract[i].address) contracts++;
}
check('site PDAs', sites === v.count, `${sites}/${v.count}`);
check('contract PDAs', contracts === v.count, `${contracts}/${v.count}`);

const ms = v.meter_and_settle;
const want = ms.accounts.map((a) => a.pubkey);
const ix = pay.meterAndSettle(want[0], want[1], want[2], want[4], want[5], want[6], ms.page_views);
const data = hex(ix.data ?? new Uint8Array());
check('meter_and_settle data', data === ms.data_hex, data);
const got = (ix.accounts ?? []).map((a) => a.address ?? a.pubkey);
check('meter_and_settle accounts', JSON.stringify(got) === JSON.stringify(want), `${got.length} accounts`);

const site = wasm.decodeSite(new Uint8Array(Buffer.from(v.site_account.data_hex, 'hex')));
const expected = v.site_account;
check(
  'decodeSite',
  site.authority === expected.authority &&
    site.mint === expected.mint &&
    String(site.pagePrice ?? site.page_price) === String(expected.page_price),
);

if (fail.length) {
  console.error(`\n${fail.length} check(s) failed: ${fail.join(', ')}`);
  exit(1);
}
console.log('\nall checks passed');
