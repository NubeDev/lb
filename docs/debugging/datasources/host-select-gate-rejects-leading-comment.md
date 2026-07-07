# Host SELECT gate rejects a leading SQL comment ("JOIN isn't allowed")

- **Date:** 2026-07-06 · **Area:** datasources / `federation.query`
- **Symptom:** `bad input: rejected sql: only SELECT/WITH allowed` — reported as "JOIN isn't
  allowed" because the failing statement happened to contain a JOIN.
- **Reproduction:** JOINs were fine — the canvas-built
  `SELECT * FROM "site" INNER JOIN "site_tag" ON …` ran live and returned 80 rows. The gate fires
  when the statement does not *literally begin* with `SELECT`/`WITH`: a leading `-- comment`
  (agents write `-- header` lines), a markdown ` ```sql ` fence, or an empty first token.
- **Root cause:** `validate_select_host` (`rust/crates/host/src/federation/validate.rs`) took the
  FIRST whitespace token as the leader — so `-- top sites\nSELECT …` had leader `--` and was
  rejected, even though the sidecar's authoritative `sqlparser` accepts leading comments. The
  host's defense-in-depth gate was STRICTER than the authoritative parser on valid input.
- **Fix:** strip leading `--` line and `/* */` block comments (loop) before the leader check. A
  comment still cannot hide a write — the leader AFTER stripping is still keyword-gated, and the
  sidecar re-validates with a real parser. An unterminated block comment / comment-only input is
  still rejected.
- **Regression test:** `federation::validate::tests::skips_leading_comments_but_still_gates_the_real_leader`
  (`cargo test -p lb-host --lib federation::validate`).
- **Note:** markdown fences are NOT stripped — they are not SQL; an agent must publish clean SQL
  (the sidecar rejects fenced text too, with an honest parse error).
