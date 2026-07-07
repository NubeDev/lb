# Template card renders "no template" — the starter snippet was never seeded

**Date:** 2026-07-06 · **Area:** frontend (panel-builder / Data Studio) · **Status:** resolved

## Symptom

Picking the **Template** card in the stacked builder's viz gallery showed an empty CodeMirror
editor and a blank preview ("no template"). The user read it as "there is no jsx code???" —
the shipped starter example (`DEFAULT_INLINE_CODE`) never appeared.

## Root cause

A freshly-picked `view:"template"` cell has neither `options.code` nor `options.templateId`.
`TemplateOptionsEditor.readValue` fell back to `{ mode: "inline", code: "" }` — an empty
editor — and `TemplateView` honestly rendered "no template". The starter was only inserted
by the `ModeTab` onClick for "Inline"… which is the tab that is **already active**, so a
user would never click it. The seed path was dead code on the default flow.

## Fix

`TemplateOptionsEditor` seeds `DEFAULT_INLINE_CODE` into `carry.extraOptions.code` via a
guarded one-shot effect when the cell has neither key. Guard: `typeof code !== "string"` —
a user-cleared editor (`""` IS a string) is never overwritten, and a `templateId` (Saved
mode) suppresses the seed.

## Regression test

`ui/src/features/panel-builder/tabs/options/TemplateOptionsEditor.test.tsx` — seeds when
unset; never patches when `code: ""` or a `templateId` is present.

## Lesson

A "default" that only materializes through an affordance the default state already occupies
is not a default. Seed initial content from the state machine (mount/first-render), not from
the toggle that selects the already-selected mode.
