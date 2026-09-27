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
| `data.title`, BM25 | the search box, server-side over every row |
| `data.tags.<key>`, BM25, one per tag key the search columns read | the search box's columns (below) |

## Search by column — what the embedder's table shows

The search box looks in the columns an embedder lists in `BootConfig::insight_search_columns`, each
the text sources its cell falls back through (`title` / `tag:<key>`; rubix-ai: Name, Site, State,
Subsystem, Category). lb names no column; empty searches the title alone. **A row matches when the
typed text appears anywhere in a column's shown value, case ignored.**

- **Candidates from `ngram(1,15)` indexes** (`insight_text_v3`): one full-text index on the title
  (`insight_name_v3`) and one per tag key (`insight_tag_<key>_v3`). The earlier `edgengram(2,15)`
  indexed word STARTS only; on a live store it missed the middle of a word (`raka`), the end of one
  (`line`), and any search holding a one-character word (`Lot 3` found 0 of 43). The v2 indexes are
  removed when v3 is defined (the planner answers from the older of two indexes on a field).
- **Then the exact check**: `string::contains` over each column's shown value (the same expression
  the column sort uses, `text_expr.rs`), so an index over-match is dropped and a source the cell does
  not show (a title behind a `short_name`) never matches. Measured: `energy` matched 144 rows on the
  title-and-tags index, 142 on what the cells show.
- **The search runs as an INNER query** (`FROM (SELECT * … WHERE title @0@ q OR tags.k @1@ q)`), and
  every filter and the exact check run over its results. Measured on 3.2.4: several `@@` joined by
  `OR` use their indexes only when nothing else is AND-ed to them; with any other predicate the
  planner falls back to `Iterate Table`. `tests/search_columns_test.rs` asserts both plans.
- **Built once per workspace**, never per request: at boot for the boot workspace, else by the first
  search, with concurrent first searches sharing one build (`host/src/insight/search_schema.rs`;
  five concurrent first searches on a fresh on-disk store used to ALL fail with a write conflict).
- **The columns never come from the wire** (`serde(skip)`, like the entity limit). The boot refuses a
  source that is not `title` / `tag:<lowercase key>`, more than 8 columns, or more than 16 tag keys.

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

## Column sort

`insight.list` takes `sort: { by: [sources], desc }`. A source is `title`, `tag:<key>`, or one of
`severity`, `last_ts`, `first_ts`, `count`, `case_stage` (the last needs a case lens). Text sources
chain (first non-empty wins, so a Name column sorts by `["tag:short_name", "tag:insight", "title"]`);
a numeric source stands alone. lb names no column.

- **One `ORDER BY` for both read paths** (`sort_sql.rs`). The counted scan used to re-sort in Rust;
  it now keeps the statement's order, so a counted page 1 and an uncounted page 2 are slices of one
  order and paging never repeats or skips a row.
- **Blanks sort last in both directions; text compares lowercased.** Ties fall back to newest first.
- **Cost is the default order's cost**: every order here was already a sort over the matched set.
- **Paged by `offset` only.** A keyset cursor walks the default order, so a cursor with a sort is
  refused.

## Risks

**The pager still reports a window as a total.** Search is now server-side, but the roster's
`total = rows.length` is unchanged. It should state the window honestly or page properly.

**BM25 is a real index with real cost**, unlike the other two — it stores terms, not just a value and
an id. Worth measuring before a workspace grows large.
