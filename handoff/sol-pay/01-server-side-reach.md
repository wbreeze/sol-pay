# The server half is Rust-only, and that is a distribution problem

Surfaced 2026-09-02 while choosing a platform for the demonstrator. Filed
against sol-pay because the demo's own choice is trivial and the question
underneath it is not.

**Status, 2026-09-04.** The conclusion, the fact that `php-client` answers the
PHP case, and the two remedies still unchosen are now recorded in
`wasm-client/SPEC.md` §3.1; the cost of keeping a port honest is in §8.1. What
remains here is the evidence behind them, kept because **the Node-tier
question is still open** and this is its brief. Delete it when that is decided,
or when it becomes an issue.

## The question

`wasm-client/SPEC.md` §3 splits the library into two consumers: a browser that
takes the npm package, and a **site's Rust server** that takes the crates.io
crate. The second half of that sentence assumes something. What language does a
site that would adopt sol-pay actually run?

## What the numbers say

Surveyed 2026-09-02.

**Server-side languages across the web** ([W3Techs](https://w3techs.com/technologies/overview/programming_language),
surveyed 1 September 2026): PHP **70.2%**, JavaScript 7.2%, Ruby 7.0%, Java
5.5%, Scala 5.1%, ASP.NET 4.2%, Python 1.2%, Go <0.1%. **Rust is not tracked at
all** — it does not appear even in the sub-0.1% tail, a list that includes Ada,
Erlang and Lasso.

**CMS market** ([W3Techs](https://w3techs.com/technologies/overview/content_management)):
WordPress 40.7% of all sites and 58.9% of the CMS market; Joomla, Drupal and
TYPO3 add another 2.2%. All PHP. Ghost, the largest Node CMS, is 0.1%.

**Publishers that already run account paywalls** — the exact population sol-pay
is proposing to serve:

| publisher | server side |
| --- | --- |
| Wikipedia / Wikimedia | MediaWiki, PHP |
| Vox Media | retired its own Ruby CMS in 2023, moved to WordPress VIP (PHP) |
| The Times, TIME, Al Jazeera, Slate | WordPress VIP (PHP) |
| Financial Times | Node / Express services; the paywall decision is its own service |
| Washington Post (Arc XP) | publisher code runs on Node 22 |
| The Guardian | Scala core, article rendering in TypeScript/Node |
| New York Times | Java (Kafka Streams), Go services |
| Substack | Node / Express |

Not one is Rust. No news publisher of any size was found running a Rust web
application server.

**Greenfield** ([Stack Overflow 2025](https://survey.stackoverflow.co/2025/technology),
49,091 respondents): JavaScript 68.8%, TypeScript 48.8%, Python 54.8%, PHP
19.1%, Go 17.4%, Rust 14.5% — where Rust's figure is *any* use of the language,
overwhelmingly not web serving. Node.js 48.7% and Next.js 20.8% among
frameworks.

## What the table above says about the Node tier

This is the part still to decide, and the publisher rows are the argument. The
PHP mass is CMS-shaped — WordPress installations, where the integration is a
plugin. The Node mass is different: the FT, the Washington Post's Arc XP, the
Guardian's rendering layer and Substack are all places where **entitlement
logic already runs in Node**, which is exactly the code a metering decision
would sit beside. That tier is reachable from this source tree with a build
flag, and reaching it requires no second implementation and no new API surface.

Against it: nothing yet, which is itself worth noticing. It has not been argued
down, only not taken up.

## Precedent worth noting

The Solana ecosystem already shows this shape. Official SDKs are Rust and
TypeScript; Python is served by `solders`, a Rust binding; Java and Go are
community ports; **PHP is unserved** — the one PHP SDK is abandoned, archived
in 2024, last released 2022. Whatever sol-pay does about PHP, it will be doing
it without an ecosystem underneath.

Sources: [W3Techs languages](https://w3techs.com/technologies/overview/programming_language) ·
[W3Techs CMS](https://w3techs.com/technologies/overview/content_management) ·
[Stack Overflow 2025](https://survey.stackoverflow.co/2025/technology) ·
[Solana client SDKs](https://solana.com/docs/clients) ·
[FT architecture](https://medium.com/ft-product-technology/making-a-request-to-the-financial-times-b2119a2f422d) ·
[Arc XP PageBuilder](https://dev.arcxp.com/pagebuilder-engine/how-to-guides/migration-guides/migrating-from-pagebuilder-engine-6x-to-7x/) ·
[Vox to WordPress VIP](https://www.axios.com/2023/07/18/vox-media-chorus) ·
[Wikimedia application servers](https://wikitech.wikimedia.org/wiki/Application_servers)
