# "Per-view revenue equals or beats targeted advertising"

Surfaced 2026-09-02, testing a claim before putting it on a public page.
Filed against sol-pay because it is the business case for the whole design,
not for the demonstrator.

**Short version: at 1¢ the claim is not defensible. A narrower claim is, and it
is still a good one. The price is the problem, not the model.**

## The comparison, publisher-net

Ad revenue per *page view* for news, not per impression — a page carries
several slots, and conflating the two is where this argument usually goes
wrong.

| comparator | $/page view | 1¢ metered is |
| --- | --- | --- |
| open-market programmatic, marginal news inventory | $0.0072 | **1.4× — holds** |
| blended incl. direct-sold, US metro daily | $0.0214 | 0.47× — fails by half |
| premium direct-sold | ~$0.04 | 0.25× — fails badly |

Sources: Lenfest Institute unit-economics study (Oct 2019) — $21.44 total RPM
and $7.16 programmatic RPM for a US metro daily, with an industry range of
$20–25 total and $6–10 "at the margins"; Operative/STAQ benchmarks via Digiday
(Feb 2023) — open-marketplace CPM $1.21 against programmatic-guaranteed $10.00,
an 8× spread.

**Weak-data warning.** The best public news-specific RPM figures are from 2019
and January 2023. Nothing credible for 2025–26 was found. Anything built on
these should be re-checked against a publisher's own numbers before it is used
to make a decision.

## The break-even that kills it

Advertising monetizes essentially every page view. Metering monetizes only the
views someone pays for. So for a page priced at *m* against ad revenue *A*, the
required payment rate is `p = A / m`:

- against programmatic $0.0072 → **p = 72%**
- against blended $0.0214 → **p = 214%**, arithmetically impossible at 1¢

Add a traffic loss *d* and it is `p = A / (m·(1−d))`. At the 51% visit drop
Chiou & Tucker measured on Gannett paywalls (2010), the programmatic case needs
**143%**. Also impossible.

Now the observed rates. Median publisher converts **0.6%** of visitors to
subscribers, top quartile 1.4% (INMA via Press Gazette, Jul 2024). Piano's 2024
benchmarks: about **1%** of visitors are known users, 66% are one-off. Reuters
Institute DNR 2025: **18% of adults across 20 richer countries** pay for online
news at all, 20% in the US, and subscription levels "now look to have hit a
ceiling."

**72% is roughly 120× the median publisher's conversion rate.** Per-view
payment should convert better than subscription — the commitment is far smaller
— but not by two orders of magnitude.

## What the price would have to be

At an opt-in rate of 20% of page views — as optimistic as the entire share of
US adults who pay for any news:

- vs programmatic $0.0072 → **$0.036 per article**
- vs blended $0.0214 → **$0.107 per article**

**3.6¢ to 11¢.** Which is exactly where the market went on its own: Blendle
charged 19–39¢ for newspaper pieces, Cornwall Reports 20p, Maidenhead
Advertiser 40p for a day pass, The New European 10p. Every real venture priced
**10× to 80× above 1¢**, and the arithmetic above says they were right to.

That convergence is evidence *for* metering as a model and *against* 1¢ as the
price.

## The targeting premium is not the argument to lean on

The tempting claim is that behavioural targeting adds almost nothing to
publishers — Marotta, Abhishek & Acquisti (WEIS 2019) found a **4% premium,
$0.00008 per impression**. Do not build on it. It is one publisher's data from
one week in May 2016, its own headline figure does not reconcile with its
reported potential-outcome means (4% vs 9.2%), and the weight of later evidence
is against it:

- Google RCT (2019, top-500 GAM publishers): −52% average, **news −62%**.
  Partial equilibrium, so overstated.
- Johnson, Shriver & Du (*Marketing Science*, 2020): opt-out impressions worth
  ~52% less. Same bias direction.
- Skiera et al. on Apple ATT: trackable ads priced **51% higher**, yet actual
  post-ATT revenue fell only **15–23%** — the cross-sectional premium is about
  3× the market-level effect, which is the reconciliation.
- **Gu, Johnson & Kobayashi (PNAS 2026)** — the largest and best identified,
  200M+ impressions across 5,000+ publishers: removing third-party cookies cut
  publisher revenue **29.1%**; Privacy Sandbox recovered 4.2% of the loss.

**Honest number: tracking is worth roughly 15–30% of publisher programmatic
revenue.** Not 4%, not 52%. A privacy argument that claims publishers lose
nothing by giving up tracking is making a claim the evidence does not support.

## The argument that does hold

**The intermediaries, not the targeting.** ISBA/PwC's two Programmatic Supply
Chain Transparency studies (data Jan–Mar 2020 and Sep–Oct 2022) both found that
**51% of advertiser spend reaches the publisher**. Between them the
unattributable "unknown delta" fell from ~15% to 3% — transparency improved and
the take rate did not move at all.

So **1¢ paid directly corresponds to about 2¢ of gross advertiser spend.** The
publisher-side comparison in the table above is already net, so do not apply
this twice — but as a statement about where the money goes it is solid, and it
is the most defensible thing in this document.

**And the reader who was never going to subscribe.** Subscriptions have hit a
ceiling at ~18–20% of adults and 0.6% of a given publisher's visitors. Metering
is not competing with the subscription; it is addressing the 99% who bounce off
it. Against *that* population the ad comparator is the marginal one — open
exchange, well under a cent — which is the tier the claim survives against.

## The thing sol-pay actually fixes, and the thing it does not

Blendle is the load-bearing precedent and its failure is usually
mis-remembered as payment friction. It was not. Klöpping's own numbers:
per-article buyers read **7 minutes a day; subscribers read 22.** Over five
years micropayments paid publishers €8m in total; for NRC Handelsblad a year of
Blendle equalled about **400 self-sold subscriptions** against a base of
241,000. Cafeyn's CEO, on shutting it down in 2023: "all-access bundles are
what paying news consumers expect — not small, individual purchases."

**The problem was the per-article decision, not the payment.** A reader asked to
choose, twenty times a day, whether this piece is worth 29¢ mostly answers no,
and stops being a habitual reader.

This is where sol-pay's architecture is genuinely different from Blendle's, and
it is worth saying plainly because it is the strongest structural argument
available. sol-pay does not ask for a decision per article. The reader
authorizes a limit **once**, and then reads with no interaction at all — the
`meter_and_settle` path never touches the wallet. Behaviourally that is much
closer to a subscription than to a newsstand, which is precisely the gap
Blendle's 7-versus-22 minutes measures.

What sol-pay cannot fix is that this remains unproven. **Google Contributor ran
per-page-view payment with the ad auction, the identity layer and the payment
rails all in-house, and abandoned it twice** (shut Jan 2017, relaunched Jun
2017, quietly dead). That is the single most damaging precedent and it should be
answered rather than ignored — the answer being that Contributor asked readers
to buy *out of* advertising on sites that kept running it, which is a different
and worse proposition than a site that does not track at all.

## Recommended positioning

Do not say: *equals or beats targeted advertising.* It does not, at 1¢, against
blended news ad revenue, and the required opt-in rate is ~100× anything
observed.

Do say: **a direct payment of a cent or two per article exceeds what a
publisher nets from open-market programmatic on marginal inventory, comes
without the ~49% intermediary take, and addresses the ~99% of readers who will
never subscribe — with a one-time authorization rather than the per-article
decision that sank every previous attempt.**

Both halves are sourced. The second is a better pitch anyway, because it does
not require the publisher to believe something they can check and disprove in
an afternoon.

## Consequence for pricing

If a real deployment prices at 1¢, the historical record and the arithmetic
both say it is **5–10× too low**. Worth deciding deliberately, since
`page_price` is a per-site parameter and nothing in the program constrains it.
The demonstrator's 1¢ is a demo figure chosen so a visitor reaches the
collection threshold in ten views, and is not a recommendation.

## Sources

[Lenfest unit economics](https://www.lenfestinstitute.org/solutions-resources/one-subscriber-or-48000-page-views-why-journalists-should-know-the-unit-economics-of-digital-news/) ·
[Digiday / STAQ CPM benchmarks](https://digiday.com/media/the-programmatic-open-marketplace-is-faltering-but-publishers-see-a-bright-spot-in-private-programmatic-deals/) ·
[Marotta, Abhishek & Acquisti (WEIS 2019)](https://weis2019.econinfosec.org/wp-content/uploads/sites/6/2019/05/WEIS_2019_paper_38.pdf) ·
[Google cookie-disabling study](https://services.google.com/fh/files/misc/disabling_third-party_cookies_publisher_revenue.pdf) ·
[Skiera et al., FTC PrivacyCon](https://www.ftc.gov/system/files/ftc_gov/pdf/3-Skiera-Economic-Impact-of-Opt-in-versus-Opt-out-Requirements-for-Personal-Data-Usage.pdf) ·
[Gu, Johnson & Kobayashi (SSRN)](https://papers.ssrn.com/sol3/Delivery.cfm/5284526.pdf?abstractid=5284526&mirid=1) ·
[ISBA/PwC I](https://www.isba.org.uk/system/files/media/documents/2020-12/executive-summary-programmatic-supply-chain-transparency-study.pdf) ·
[ISBA/PwC II](https://www.isba.org.uk/system/files/media/documents/2023-01/ISBA%20%20PwC%20programmatic%20supply%20chain%20study%20II%20(summary)-%2018%20January%202023.pdf) ·
[INMA conversion via Press Gazette](https://pressgazette.co.uk/media-audience-and-business-data/newsbrand-subscriber-conversion-rates-biggest-reader-funded-newsbrands-ranked/) ·
[Piano Subscription Benchmarks 2024](https://www.piano.io/marketing/content/subscription-performance-benchmarks-2024) ·
[Reuters Institute DNR 2025](https://reutersinstitute.politics.ox.ac.uk/digital-news-report/2025/dnr-executive-summary) ·
[Chiou & Tucker, paywalls](https://www.oxy.edu/sites/default/files/assets/Economics/Chiou/chiou_and_tucker_paywalls.pdf) ·
[Blendle pivot, Nieman Lab](https://www.niemanlab.org/2019/06/micropayments-for-news-pioneer-blendle-is-pivoting-from-micropayments/) ·
[Blendle exit, Nieman Lab](https://www.niemanlab.org/2023/08/the-poster-child-for-micropayments-for-news-is-getting-out-of-the-micropayments-business/) ·
[UK micropayments, Press Gazette](https://pressgazette.co.uk/paywalls/micropayments-for-news/)
