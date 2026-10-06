# Being a good citizen of QLever

Everything here is about one property of the endpoint: **it is a shared,
publicly funded resource, and it is generous right up until it is not.**

QLever hosts the LOTUS Wikidata index for anyone, with no key and no
registration. There is no contract and no quota we can read, so the only
defensible posture is the one a well-mannered API client takes when it cannot
see the other users: declare what it is, ask for less than it can give, and
never make it guess whether you are worth keeping.

## A 429 is a timeout, not a rate limit

This is the single most important fact on this page, and it was wrong here for
months.

`QLever` answers **`429` when a query exceeds its time limit.** It is not a
request-rate refusal. From QLever's own source (`src/engine/Server.cpp`):

```cpp
} catch (const ad_utility::CancellationException& e) {
  // Send 429 status code to indicate that the time limit was reached
  responseStatus = http::status::too_many_requests;
```

Measured against `https://qlever.dev/api/wikidata` on 2026-10-04:

| Request                                     | Answer                                                     |
| ------------------------------------------- | ---------------------------------------------------------- |
| `timeout=3s`                                | **429** `{"exception": "Operation timed out. Last operation: Sort (internal order) on ?r"}` |
| `timeout=60s`                               | **403** `"User submitted timeout was higher than what is currently allowed by this instance (30s). Please use a valid-access token to override this server configuration."` |

So a 429 is a measurement that **one query was too expensive**, and the only
thing that answers it is a smaller query.

Retrying it is the one response that cannot help, and this workspace used to do
it — twice inside `execute` (`MAX_ATTEMPTS = 2`) and once more as a whole
pipeline retry. One broad search could therefore spend the endpoint's entire
30-second budget **four times**, which is precisely how a client becomes the kind
of client an operator blocks. `429` is now out of the retryable set, and it
carries as [`FetchError::TimedOut`] rather than as a status:

| Layer                                    | Was                                    | Now                                    |
| ---------------------------------------- | -------------------------------------- | -------------------------------------- |
| `execute` immediate retry (`MAX_ATTEMPTS`) | retried a 429, immediately on wasm     | **not retried**                        |
| WDQS fallback (`is_endpoint_unavailable`)  | did not fire                            | still does not fire                    |
| pipeline retry (`error_recovery_coordinator`) | one whole-pipeline retry, 1 s backoff | **no retry**                           |
| UI hint                                                       | "wait about a minute and retry"        | "narrow it with a taxon, mass range, year or formula" |

A `WDQS` fallback is refused for this class on purpose. `WDQS` will run the same
expensive query; falling back would answer the same question twice, on two
public endpoints, for no gain.

## Ask for less than the endpoint's ceiling

Every `QLever` request now carries `timeout=30s`. Measured, not assumed: the
public instance allows 30 s, and asking for more than that is a `403` rather than
a longer query.

Five seconds of headroom is deliberate. A query that is going to be cancelled has
already spent the endpoint's budget; cancelled at 25 s it costs five seconds less
and returns the same refusal, sooner, and with the endpoint's own account of
which operation was still running (`Last operation: Sort … on ?r`) instead of a
bare status.

`LOTUS_QLEVER_TIMEOUT` overrides the budget in QLever's own duration syntax
(`30s`, `1500ms`, `1min`) for a deployment that has an access token and a raised
ceiling. It is clamped to 30 s locally, because a request that is certain to be
refused should not be sent.

## Say who you are

Two headers go out with every request:

- **`api-user-agent`** — `lotus-explore-rs/<version> (+https://github.com/lotusnprod/lotus-explore-rs)`.
  It is the header QLever names in its own CORS allow-list. The point is not
  politeness theatre: a deployment running this app heavily should be something
  an operator can *contact and raise a limit for*, rather than an anonymous
  address that eventually gets blocked.
- **`Authorization: Bearer <token>`**, when `LOTUS_QLEVER_TOKEN` is set. This is
  how to be given more than the anonymous budget, and it is the answer to "this
  workload is legitimately heavy": **a token, not a faster retry loop.** The
  variable is read per request, so a token can be rotated without a restart.

  QLever reads a token from that header or from an `access-token` parameter, and
  refuses with a 400 if both are given and differ. There is no `api-token`
  header: an earlier version of this document described one, and it was never
  read by the server — measured against `qlever.dev`, a request carrying a token
  that way is answered identically to one carrying none.

  A token is operator-granted, not self-service. The server is started with
  `--access-token=<secret>`, so there is nothing to sign up for: an operator
  decides who gets one. Which is also why the `api-user-agent` header above
  matters — it is how the request that needs a limit identifies itself to the
  person who can grant it.

### What a token does and does not buy

It raises the ceiling on the whole request. QLever compares the `timeout` you
submit against the server's configured default and answers anything larger with
a 403 unless the token checks out; the value you submit then becomes the actual
limit. **There is no parameter that budgets computation separately from
transfer.** A query that computes in 10 s and takes 20 s to send is inside a
30 s budget and outside a 25 s one, and no setting separates the two phases.

So for a large result set the options are a token (more budget for the whole
request), fewer rows, or a narrower query — and asking for a smaller `timeout`
makes a transfer-bound query fail sooner without making it faster.

## The query that trips it

`taxon="*"` and an empty taxon box both ask for the whole dataset. Measured
against `qlever.dev`, uncached:

| Query                                   | Bytes      | Rows     | Wall   |
| --------------------------------------- | ---------- | -------- | ------ |
| `Gentiana lutea` (`Q21754`)             | 16 MB      | 32,161   | 5.1 s  |
| everything (`*`)                        | **110 MB** | **265,266** | **~30 s** |

The second one is the whole problem: it finishes at about the endpoint's
ceiling, so whether it is answered or cancelled depends on cache state. That is
the query this page exists for, and it is why:

- the unconstrained notice now states the cost in numbers rather than saying
  "this scans the whole of LOTUS", which reads as a footnote; and
- a cancellation is presented as "narrow it", never as "retry".

The browser path asks for every row with no `LIMIT`, deliberately: a server-side
truncation means the client-side filters can only ever see the truncated set. That
is the right trade for a taxon-scoped search and the wrong one for an
unconstrained one, which is what makes the confirmation question worth asking of
a human before the query is fired.

## Curation: the heaviest thing here, now bounded

Curation used to be the worst of it by an order of magnitude. Every row asked
four questions of its own — does this `InChIKey` exist, what is this taxon, what
is this DOI, is this occurrence recorded — so a 200-row import was up to **800
POSTs**, and the second pass over dependency rows repeated every one of them.

All four questions are per row and all four have a batched form, so
`services/prefetch.rs` asks each of them once, before the rows are walked:

| Question                    | Was            | Now                                                     |
| --------------------------- | -------------- | ------------------------------------------------------- |
| compounds by `InChIKey`     | 1 per row      | 1 per run, `VALUES ?key` with `MIN(?compound)` for ties  |
| taxon names                 | 1 per row miss | 1 per run, both spellings of each name                   |
| DOIs                        | 1 per row miss | 1 per run                                                |
| "is this occurrence recorded" | 1 `ASK` per row | 1 query for the pairs that *exist*; a miss is the answer |
| "...by this paper"          | 1 `ASK` per row | 1 query, same trick                                       |

**Five queries per run, whatever the row count.** The test that says so is
`a_prefetch_costs_the_same_however_many_rows_there_are`: it runs the same
prefetch at 3 rows and at 200 and asserts the request count did not move.

Two details that are load-bearing:

- **Misses are cached, not just hits.** A batch answers "which of these pairs
  exist", so the pairs that did not come back are the `false`s. A cache that
  stored only hits would send the row loop to the network for every pair
  Wikidata does *not* have — which is most of a curation run, since a run is
  mostly work still to do.
- **The taxon batch asks for both spellings.** The single-row lookup has always
  tried the canonicalised label as well as the raw one; a batch that only tried
  the raw one sent every row needing canonicalisation down the per-row path to
  be asked the same question again.

### What is left

- **A second pass rebuilds its prefetch.** `run_second_pass` calls the pipeline
  afresh, so it re-asks the batch questions rather than reusing them. That is
  three queries, not the four-per-row it was, which is why it was left: the
  remaining win is small and the change threads a run context through the page
  controller.
- **A row whose taxon the batch cannot resolve still costs one query**, in
  `resolve_or_create_taxon`. That is the honest case: the batch asked, and the
  answer was "no such item", and the row loop needs to know that individually
  because it may have to *create* the taxon.
- **Rows are still curated one at a time**, deliberately. The endpoint is the
  shared resource, not this browser's main thread.

## What is deliberately *not* here

- **No rate limiter.** There is nothing to rate-limit against: the 429 we were
  getting was not about how many requests arrived. A token bucket would have been
  a solution to a problem that did not exist, and would have made curation
  imports (deliberately serial, several queries per row) slower for no benefit.
- **No jittered retry storm.** The retry policy is now three attempts for genuine
  transport failures and zero for everything else. Every additional retry of a
  query the endpoint has already refused is load with no possible payoff.

## The measurement, and how to repeat it

```bash
# What a broad query actually costs, and whether it is answered at all.
cargo run -p lotus-cli -- search --taxon '*' --explain > /tmp/star.sparql
curl -s -o /dev/null -w '%{http_code} %{size_download}\n' \
  -X POST https://qlever.dev/api/wikidata -H 'Accept: text/csv' \
  --data-urlencode "query@/tmp/star.sparql"

# That a 429 is a cancellation, not a request-rate refusal.
curl -s -X POST https://qlever.dev/api/wikidata \
  -H 'Accept: application/sparql-results+json' \
  --data-urlencode "query@/tmp/star.sparql" --data-urlencode 'timeout=3s'

# That the ceiling is 30 s.
curl -s -X POST https://qlever.dev/api/wikidata -H 'Accept: text/csv' \
  --data-urlencode "query@/tmp/star.sparql" --data-urlencode 'timeout=60s'
```

Two queries and a form field, against a public endpoint, a handful of times.
That is the budget this project spends on other people's hardware.
