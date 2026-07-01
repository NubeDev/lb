# `srcdoc` template literal broke on a backtick inside an embedded comment

## Symptom

After embedding the template interpolator into the iframe `srcdoc` builder, `tsc --noEmit`
failed with:

```
src/features/dashboard/builder/iframeRuntime.ts(43,10): error TS2349:
  This expression is not callable. Type 'String' has no call signatures.
```

Line 43 is the opening of the `buildIframeSrcdoc` return **template literal** — nowhere near
an obvious call site, which made the error look bogus.

## Cause

The embedded `<script>` block carried a `//` comment that used backticks as inline code
quotes, e.g. `` …so `.toString()` is complete… ``. Those backticks live **inside** the outer
`` return `…` `` template literal, so the first one **terminated the template string early**.
The text after it (`.toString()…`) was then parsed as real code — `"<string>".toString()`
followed by more stray tokens — and TS reported the confusing "Type 'String' has no call
signatures" at the template-literal start.

A `//` comment does **not** protect backticks inside a template literal — the literal is
tokenized before comments within the interpolated text mean anything.

## Fix

Reword the embedded comment to avoid literal backticks (no code-quoting inside the srcdoc
string). Same discipline already applies to `</script>` and `${` inside these embedded
scripts — treat backtick, `${`, and `</script>` as the three sequences that must never appear
raw in a `srcdoc` template literal.

Commit: template-widget data-binding session.

## Regression guard

`tsc --noEmit` catches this immediately (the whole file fails to parse), so the typecheck
step in CI is the guard — no separate test needed. If we add more embedded-script builders,
keep their inline comments backtick-free.
