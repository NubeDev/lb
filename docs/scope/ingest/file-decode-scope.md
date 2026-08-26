# Ingest scope — file decoders (`lb_ingest::decode`): a file becomes samples

Status: **SHIPPED (v1) 2026-08-26** — branch `feat/mail-source-ingest`, unreleased. Built as part of
[`../inbox-outbox/mail-source-scope.md`](../inbox-outbox/mail-source-scope.md), whose first consumer
it is. Session: [`../../sessions/inbox-outbox/mail-source-session.md`](../../sessions/inbox-outbox/mail-source-session.md).

> Read with: `ingest-scope.md` (the `Sample` envelope and the write path this feeds),
> `series-normalize-scope.md` (the *other* shaping question — normalizing a series after it lands,
> not getting it in), README §3 rules 9/10.

## The problem

A `Sample` is the platform's one data-plane envelope, and until now **everything that produced one
did so by speaking JSON at `ingest.write`**. A file did not.

That is a real hole, because a very large amount of real-world data arrives as a file: a meter
export emailed monthly, a logger dump on a USB stick, a historian's CSV, a spreadsheet a human
saved. Every one of those needed a bespoke parser somewhere — in a product, in a script, in an
extension — and each one re-invented the same three decisions (what is a series here, what is the
timestamp, what is the dedup key) differently.

## Goals

- **One entry point.** `decode(format_id, input, options) -> Decoded` in `crates/ingest/src/decode/`.
  `input` is `{filename, mime, bytes}`; `Decoded` is `{format, samples, series, warnings, truncated}`.
- **The format id is opaque.** Resolved through a `FORMATS` table; **no caller branches on it.** A
  new format is a new file in the folder and one row in that table — not a change to the mail
  source, the ingest verb, or the MCP surface.
- **Decoders are pure.** Bytes + options in, samples out. No store, no clock, no network, no
  workspace. Every decoder is exercisable from a byte literal, which is why the tests run against the
  real files this shipped for rather than hand-shaped snippets.
- **`detect()`** identifies a format from the bytes first and the extension second.
- **Bounded.** A file is untrusted input; a malformed or hostile one can describe tens of millions of
  points. `DecodeOptions::max_samples` caps a decode and `Decoded::truncated` says so out loud.

## Non-goals

- **A schema/mapping language.** A header is a series name. Encoding a per-file column map here grows
  into a DSL; the two shipped shapes (a nested record format, a wide grid) already cover the cases a
  map is usually reached for, and `series.rename` exists.
- **Type inference beyond number/bool.** The series plane's readers are numeric; quietly writing
  `"n/a"` as a payload produces a chart with an unexplained hole.
- **Streaming.** A decode holds the file and its samples in memory, bounded by the ceiling above.
  A gigabyte historian export is a different slice.
- **Writing.** Decoders never touch the store; the caller hands the samples to the gated
  `ingest.write`.

## Decisions worth stating

**1. `seq = ts_ms / 1000` — derived from the instant, never from file order.** This is the whole
design. `seq` is half of ingest's dedup identity `(series, producer, seq)`. File-order numbering is
correct for re-importing the *same* file and silently destructive for a **second** file covering an
overlapping period — a corrected re-issue, or a monthly export that repeats the last week — which
would reuse `seq 0..N` for different instants and overwrite real data. Deriving from the timestamp
makes the identity a property of the instant: re-imports are exact upserts, overlapping files
converge, and `series.latest` (highest seq) still means newest. Sub-second data would collide; no
shipped format produces it, and one that did must pick a finer derivation rather than falling back
to file order.

**2. An error means *no* samples; a bad row means *fewer*.** A month of interval data with one
unparseable cell must import the other 4,319 points. Only a file that is not what it claims fails.
Warnings ride back on a successful decode so the caller can see exactly how many rows were lost and
why.

**3. Timezone is caller configuration, never inferred.** `DecodeOptions::offset_minutes`. NEM12 times
are NEM time (UTC+10, no DST) *by specification* and the file says so nowhere; a CSV from a
spreadsheet is in whatever zone the exporter's laptop was in. Guessing shifts a month of data by an
hour. Where a file *does* state its zone (an ISO 8601 offset), the file wins.

**4. A named file format is not an "extension" (rule 10).** Rule 10 says core knows no extension. A
file format is a way bytes are shaped, like JSON or base64, and the platform already owns the data
plane those bytes become. The test that keeps it honest: **nothing outside `decode/nem12.rs` knows
NEM12 exists.** Deleting the file removes a format; it changes no caller.

**5. No `chrono`.** The decoders need one direction of one conversion — a `YYYY-MM-DD hh:mm:ss` the
file spells out, into epoch millis. That is Hinnant's `days_from_civil`, twenty lines, in
`decode/civil.rs`. Pulling a date/time library into the data plane to avoid writing them would be a
dependency taken for a rounding of convenience. There is deliberately **no tz database**: named
zones with DST are a policy question the caller answers with `offset_minutes`.

## Shipped formats

| id | what |
|---|---|
| `nem12` | AEMO NEM12 interval metering (`100`/`200`/`300` records). One series per `(NMI, suffix)`; values are **period-ENDING** (interval *i* covers `[(i-1)·L, i·L)` and is stamped at `i·L`); the meter's own dimensions (`nmi`, `suffix`, `uom`, `meterSerial`, `intervalMinutes`, `quality`) become labels. A `300` under a broken `200` is dropped, never attributed to the previous meter. `400` per-interval quality events are parsed past, not modelled — that needs a per-sample quality dimension the series plane does not have. |
| `csv-grid` | A timestamp column, then one series per remaining column, named by its header. Timestamps accept ISO 8601 (with or without a zone), epoch seconds, and epoch millis (disambiguated by magnitude at 1e11, exact for every instant between 1973 and the year 33658). |

## How it fits the core

- **Tenancy:** none — a decoder never sees a workspace. The caller writes the samples through the
  gated `ingest.write` in its own.
- **Capabilities:** none. `mail.formats` reads the registry out (a static property of the binary),
  riding the roster read cap rather than minting one for a constant list.
- **Data:** none. Decoders produce values.
- **No mocks (rule 9):** the fixtures are **real files** — the shipped test runs against a genuine
  163 KB four-channel NEM12 export, which is where the blank `RegisterID`, the mid-file `400`
  records, the `V` quality method, and the trailing comma on a `200` come from. A snippet written
  from the spec has none of those, and those are what a spec-written parser gets wrong.

## Testing

`decode_test` (13, against the real export) + 19 unit. The two that matter most:
`an_overlapping_re_issue_lands_on_the_same_dedup_keys` (decision 1) and
`rows_under_a_broken_header_are_never_attributed_to_the_previous_meter` (silent data corruption).
Verified live end to end via the mail source — see the session doc's bucketed read, whose shape
(a solar curve peaking at midday) independently validates the offset and the period-ending
convention in a way no unit test can.

## Open questions

- **Where does a format that is genuinely product-specific live?** NEM12 is defensible in core (a
  file format, not a product), but a decoder for one customer's bespoke logger is not. The seam
  admits an out-of-core answer — an extension exposing `<ext>.decode` that the mail source calls by
  its opaque format id — but nothing implements it yet, and doing so needs the extension call to be
  reachable from the importer's deliberately narrow principal.
- **Per-format options.** `DecodeOptions` is shared by every decoder today. A format needing its own
  knob (a header row offset, a delimiter) will want a typed per-format block rather than more shared
  fields.
- **Streaming / chunked decode** for files past the sample ceiling.
- **Should `detect()` be reachable as a verb?** A "what is this file?" answer is useful to a UI
  before an import is configured; today only `mail.formats` (the static list) is exposed.
