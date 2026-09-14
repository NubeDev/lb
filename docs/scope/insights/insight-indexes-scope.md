# Insights scope — indexes, name search, and what pagination can actually do

Three asks: real (server-side) search, indexed pagination, and the indexes behind them. The third one
is where the surprises were, so every claim below is a MEASURED plan or timing from SurrealDB 3.2.4 /
surrealkv 0.21.4, not a design intention.

## The problem, measured

**Every raise is a full table scan today.** `insight_id::dedup_lookup` runs
`SELECT data FROM insight WHERE data.dedup_key = $v` on every single raise, and the engine's own
`EXPLAIN` answers:

```
{"operator":"TableScan","predicate":"data.dedup_key = 'x'","table":"insight"}
```

Insight documents average 3,016 bytes on the shipped ESR data, which is over surrealkv's 1 KB value
threshold, so they live in the value log. The scan therefore reads every document off disc to answer
one equality — ~594 KB per raise at 197 rows, and rules fire every 15 minutes. Nobody sees this cost;
it is not the page load people complain about.

**The roster searches a window, not the table.** The search box filters rows already in the browser
and the pager reports `total = rows.length` — the size of the window, stated with the confidence of a
real total.

## What the engine will and will not do

Probed on seeded tables of 5,000 and 20,000 rows.

**Indexes on nested `data.*` paths ARE used.** This was the real risk: every lb record is wrapped as
`{ data: <host json>, rev: n }`, so every index we can add is on a nested path.

```
equality, before:  SelectProject → TableScan(insight)
equality, after:   SelectProject → IndexScan(insight_dedup)
```

**`ORDER BY` cannot be made index-ordered.** `SortTopKByKey` is present in EVERY shape tried:

| shape (20,000 rows) | time | plan |
|---|---|---|
| `ORDER BY data.last_ts DESC LIMIT 50` | 99.67 ms | TableScan + sort |
| keyset, 19,950 rows below cursor | 173.22 ms | IndexScan + sort |
| keyset, 10,000 rows below cursor | 86.75 ms | IndexScan + sort |
| keyset, 100 rows below cursor | 1.55 ms | IndexScan + sort |
| compound keyset (the real cursor) | 156.55 ms | UnionIndexScan + Filter + sort |
| `WITH INDEX` hint | — | **ignored**, still TableScan |
| `DEFINE INDEX … DESC` | — | **rejected by the parser** |

So cost tracks the rows the predicate MATCHES, not `LIMIT`. For newest-first paging that means page 1
is the worst case, and an index on `last_ts` made it *slower* (47 ms vs 28 ms at 5,000 rows) because
it adds indirection and removes no work.

**Ordering by the PRIMARY KEY is free.** Records are stored in key order, so the engine skips the sort
entirely:

```
ORDER BY data.last_ts DESC LIMIT 50   99.67 ms   TableScan → SortTopKByKey
ORDER BY id          DESC LIMIT 50     0.70 ms   TableScan            ← no sort step
```

140× faster on the same rows, with no secondary index to maintain. The insight id is already a ULID,
whose leading bits are its creation time, so **id order is time order**.

## Full text on SurrealDB 3 — two things that cost an hour

**The keyword changed.** SurrealDB 2.x spells it `SEARCH ANALYZER … BM25`; 3.x spells it
`FULLTEXT ANALYZER … BM25` and rejects the old form outright with a bare
`Parse error: Unexpected token`. The parser is the authority: the arm accepting `ANALYZER` / `BM25` /
`HIGHLIGHTS` is `t!("FULLTEXT")`, and `SEARCH` appears nowhere in it.

```
DEFINE INDEX i ON TABLE t FIELDS data.title SEARCH   ANALYZER az BM25;   REJECTED (2.x form)
DEFINE INDEX i ON TABLE t FIELDS data.title FULLTEXT ANALYZER az BM25;   ACCEPTED
```

**Nested paths work.** `FIELDS data.title` is accepted and matched, which was the open risk — every
lb record is wrapped as `{ data: <host json>, rev: n }`:

```
SELECT … WHERE data.title @@ 'flatline'  → 2 rows
plan: Iterate Index (index i_ine, operator @@)
```

**A full-text plan reports differently.** It shows as `Iterate Index` under an `operation` key, not
`IndexScan` under `operator`. Anything asserting on plans has to read both.

## A search box searches by PREFIX, and the first analyzer did not

`TOKENIZERS blank FILTERS lowercase` indexes WHOLE WORDS, so typing `hi` found nothing while `high`
found the row. That is not how anyone uses a search box. Measured on ESR-shaped titles:

| analyzer | `hi` | `high` | `mdb` | `msb1` |
|---|---|---|---|---|
| `lowercase` | **0** | 1 | 0 | — |
| `lowercase, edgengram(2,15)` | **1** | 1 | **1** | **1** |

`edgengram(2,15)` indexes every 2-to-15 character prefix of each token, so a prefix matches while
whole words still do.

**`blank` stays the only tokenizer, on evidence.** Adding `class` (which splits letters from digits)
looks sensible for `MDB-1-4` but **breaks `msb1`** — it becomes `msb` + `1`, so typing the thing
printed on the panel finds nothing. And it is unnecessary: `blank` alone already matches `mdb`
inside `MDB-1-4`, because the prefix filter runs across the whole hyphenated token.

| tokenizers | `mdb` | `msb1` |
|---|---|---|
| `blank` | 1 | 1 |
| `class` | 1 | **0** |
| `blank, class, punct` | 1 | **0** |

**What it costs.** Prefix indexing is not free — this is the one index here that is genuinely large,
against ~50 bytes per row for the others:

```
2,000 rows      plain  4,108 KB
             edgengram 10,252 KB   (2.5x)
```

## The names are versioned, and that is load-bearing

The DDL is `IF NOT EXISTS`, which is a no-op once the name exists — so a workspace that already
defined the old analyzer would keep it **forever** and never see a corrected definition. This is not
hypothetical: the local ESR store defined the whole-word analyzer during verification and had to be
migrated.

So the analyzer and its index carry a `_v2` suffix. Changing the definition means bumping the suffix,
which makes it a genuinely new object, created on the next raise. The alternative — `OVERWRITE` —
would rebuild the entire index on every single raise.

**A bump alone is not enough — found the hard way, on the real store.** With two full-text indexes
on the same field, the planner picks the OLDER one. After `insight_name_v2` was defined, a live
search behaved like this:

```
search 'h'     → 5 rows   (below the 2-char floor: no filter, correct)
search 'hi'    → 0 rows   ← the v1 WHOLE-WORD index was still answering
search 'high'  → 5 rows
search 'mdb'   → 0 rows
plan: Iterate Index (index: insight_name)   ← not insight_name_v2
```

So `ensure_insight_schema` now issues `REMOVE INDEX IF EXISTS insight_name` before defining the new
one. Retiring the predecessor is part of the migration, not an afterthought.

## Paging: OFFSET beats the cursor here, which is the opposite of the usual advice

The roster's pager offers first · prev · `N of M` · next · last. Four of those five need RANDOM
access, which a keyset cursor cannot serve — it only walks forward one page at a time.

Normally offset is the wrong tool because skipping N rows costs more the deeper you go. Not on this
engine, because `ORDER BY` sorts the matched set on every request regardless (see above), so offset
merely discards from an already-sorted set. Measured at 20,000 rows, page size 20:

| page | `START` | time |
|---|---|---|
| 1 | 0 | 97.71 ms |
| 10 | 180 | 96.25 ms |
| 100 | 1,980 | 103.94 ms |
| 500 | 9,980 | 102.33 ms |
| **last** | 19,980 | **101.76 ms** |

Flat. The cursor, by contrast, swings with how many rows sit below it — 166.85 ms near the newest
end, 1.77 ms near the oldest.

And `page N of M` needs a true total, which is cheap: `count() GROUP ALL` is 3.99 ms.

## Decisions

**Two indexes, both justified by a measurement:**

| index | why |
|---|---|
| `data.dedup_key` | turns the every-raise scan into `IndexScan`; ~3% of data size |
| `data.title`, BM25 | the search box, server-side over every row; NAME ONLY by decision |

**No index on `status` or `severity`.** Three values each, matching 39–64% of rows — a scan beats an
index at that selectivity.

**No index on `last_ts`.** Proven above to buy nothing: the sort stays either way.

Defined idempotently per workspace by `ensure_insight_schema`, called from the raise path (the
`ensure_series_schema` idiom — nothing at boot knows which workspaces exist) and from the read path
when a search term is present, since `@@` errors without the index.

## Open — needs a product decision

Ordering by `id` (first seen) is free and truly bounded; ordering by `last_ts` (last activity) costs a
sort that grows with the table. That is "newest fault" versus "most recently active", a product call,
not a technical one. Until it is made, ordering stays as it is — correct, and fine at today's size.

## Risks

**The pager still reports a window as a total.** Search is now server-side, but the roster's
`total = rows.length` is unchanged. It should state the window honestly or page properly.

**BM25 is a real index with real cost**, unlike the other two — it stores terms, not just a value and
an id. Worth measuring before a workspace grows large.
