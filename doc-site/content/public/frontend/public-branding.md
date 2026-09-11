# Pre-auth branding — a branded sign-in screen on a first-ever visit (shipped 2026-09-03)

A workspace's brand is admin-owned workspace identity, stored as the `ui_branding` blob on the
workspace-default prefs record (with the workspace's `ui_theme` beside it). Authenticated, the shell
reads both through `prefs.resolve`. **Unauthenticated, it could not read them at all** — every
`prefs.*` verb takes its workspace from the bearer token — so a sign-in screen could only repaint a
brand that some *earlier* authenticated visit on that same browser had cached. On a new device, a new
browser, or a cleared profile there was nothing to paint, and a branded deployment's first impression
was the compiled product default.

`GET /public/branding` closes that gap.

## The route

```
GET /public/branding?ws=<workspace>          # no bearer, no cookie, no capability
200 { "ui_branding": <blob|null>, "ui_theme": <blob|null> }
Cache-Control: public, max-age=60
```

Both blobs are opaque to the node — the shell's branding and theme layers parse them. Both are
`null` when the workspace has set no default.

`ws` is **required**. There is no "this node's own workspace" fallback: finding one means enumerating
workspaces for an anonymous caller. A sign-in screen always knows which workspace it is signing into
(a `#/t/<ws>` deep link, the workspace on the form, or the last workspace for the typed email), so it
can always name it.

## What it will not do

This is a deliberate, opt-in, **read-only** break in the workspace wall — the same posture as the
document store's public published-doc serving. It is kept hairline by four properties, each with a
test that fails if it goes:

1. **The whitelist is construction, not filtering.** The handler destructures the prefs record into
   two named fields and builds the body from those; it never serializes the record. A future prefs
   axis cannot reach the public internet by merely existing.
2. **It is not a workspace-existence oracle.** An unknown workspace, an unbranded workspace, a
   malformed slug and an absent `ws` all return the byte-identical `200 {"ui_branding":null,
   "ui_theme":null}`.
3. **It reads only the workspace-default link.** Member prefs are not in reach even in principle.
4. **It is rate-limited per client** (60/min, keyed on the first `x-forwarded-for` hop) on its own
   budget, so a login repaint can never spend the ceiling that protects the invite routes.

Writing a brand is unchanged and still fully walled: the admin-gated `prefs.set_default`
(`mcp:prefs.set_default:call`). There is no public write half.

## Using it from a shell

Paint the cached brand first and let this response correct it — the fetch then never costs a blank
frame, and a browser that has signed in before behaves exactly as it did.

Scope: `docs/scope/frontend/workspace-branding-scope.md`; session:
`docs/sessions/frontend/public-branding-route-session.md`.
