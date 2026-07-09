# Shipped docs (`public/`)

Author the **durable, reader-facing** documentation here as MDX (`.mdx`) — one
file per shipped surface, under a topic subfolder (e.g. `flows/flows.mdx`).

This is the promotion target for `docs/scope/` + `docs/sessions/` work: when a
slice ships, promote its trimmed truth into `doc-site/content/public/`. This is
the authoring location **and** what the Nextra site renders — they are the same.

Do **not** write shipped docs under `docs/public/` — that folder is gone (it was
previously a symlink here and is no longer used). See `docs/ABOUT-DOCS.md`.
