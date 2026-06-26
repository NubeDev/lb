# FILE LAYOUT — one responsibility per file

This is the most important rule in the project for AI-assisted work.
Small, well-named files are not a style preference. They are how an
AI (or a human reading cold) finds the right code without burning
context on irrelevant lines.

> **Companion doc (planned):** `HOW-TO-CODE.md` §0 will carry the short
> version; this file is the long-form reference with patterns,
> heuristics, and worked examples.

---

## 1. The hard limits

| Limit | Value | Hard? |
|---|---|---|
| Lines per file | **400** | Hard. PR blocked above this. |
| Lines per file (warning) | 300 | Soft. Plan the split. |
| Lines per function | 50 | Soft. Extract a sub-function. |
| Public items per module | ~10 | Soft. Split the module. |
| Nesting depth | 4 | Soft. Early return, extract. |

400 lines is the **ceiling**, not the target. Most files in this
repo should sit between 30 and 150 lines.

---

## 2. The verb-per-file pattern

Group code by **the verb the caller performs**, not by the noun it
operates on. One file = one verb (or one phase of one verb).

### Example — REST tool implementations

**Wrong** — one file per noun:

```
crates/rubix-tools/src/
  user.rs          ← 600 lines: get + list + create + update + delete
                     plus password hashing, plus email validation,
                     plus the events emitted on each verb
```

**Correct** — one file per verb:

```
crates/rubix-tools/src/user/
  mod.rs           ← re-exports + module doc, ≤30 lines
  get.rs           ← GET   /users/:id      — dispatch only
  list.rs          ← GET   /users          — pagination + filter
  create.rs        ← POST  /users          — input validation, hash, persist, emit
  update.rs        ← PATCH /users/:id      — partial update
  delete.rs        ← DELETE /users/:id     — cascade rules
  error.rs         ← user-domain errors    — only if needed
```

`mod.rs` is a barrel and nothing else:

```rust
//! User goal — tool implementations.

mod create;
mod delete;
mod get;
mod list;
mod update;

pub use create::create_user;
pub use delete::delete_user;
pub use get::get_user;
pub use list::list_users;
pub use update::update_user;
```

### Why an AI prefers this

| Task | One-file user.rs | Verb-per-file folder |
|---|---|---|
| "fix create-user validation" | Loads 600 lines. ~80 are relevant. | Opens `create.rs`. 100% relevant. |
| "what does delete do to child rows?" | Grep `delete`, scan 30 hits. | Open `delete.rs`. Read top to bottom. |
| "add tests for update" | Find the right `#[test]` in a giant test mod. | Create `update.rs`'s sibling test file. |
| Two engineers edit user concurrently | Same file → merge conflict. | Different files → no conflict. |

### Why a human prefers it too

A new engineer reading `user/` learns the API by reading the
filenames before opening a single file. That is a property no
naming convention inside a 600-line file can match.

---

## 3. When the verb itself is too big

If a verb's file approaches 200 lines, split it by **phase of the
verb**, not by reusable helper noun.

```
user/create/
  mod.rs           ← orchestrates the four phases, ≤50 lines
  validate.rs      ← input checks
  hash.rs          ← password derivation
  persist.rs       ← DB insert
  emit.rs          ← post-create event
```

Each filename is a **searchable phase verb**. Never `helpers.rs`,
`utils.rs`, `internal.rs`, `support.rs`. If the only honest name
for a file is "miscellaneous things create needs", the boundary is
wrong — those pieces belong inside `validate.rs` / `persist.rs` /
wherever they're actually used, or in a higher-level shared module
with a real name.

---

## 4. Other layout patterns by code shape

### Transport routes

One file per HTTP route, grouped by resource folder.

```
crates/rubix-transport/src/users/
  mod.rs               ← Router::new().merge(...) wiring only
  get.rs               ← GET   /users/:id
  list.rs              ← GET   /users
  create.rs            ← POST  /users
  update.rs            ← PATCH /users/:id
  delete.rs            ← DELETE /users/:id
```

Each handler ≤20 lines: extract input → call domain → map DTO → return.

### Domain state machines

One file per state transition.

```
device/state/
  mod.rs               ← the FSM enum + transition table
  connect.rs           ← offline → connecting
  online.rs            ← connecting → online
  fault.rs             ← * → fault
  recover.rs           ← fault → online
```

### DTOs (wire types)

DTOs follow the verbs they describe. Don't pre-split a tiny module,
but do split once it grows.

**Small (≤150 lines for the whole noun) — one file is fine:**

```
rubix-spi/src/dto/user.rs        ← all user request/response types
```

**Once it grows — split by verb, like the tool side:**

```
rubix-spi/src/dto/user/
  mod.rs               ← re-exports
  get.rs               ← GetUserRequest, GetUserResponse
  list.rs              ← ListUsersRequest, ListUsersResponse, Cursor
  create.rs            ← CreateUserRequest, CreateUserResponse
  update.rs            ← UpdateUserRequest, UpdateUserResponse
  delete.rs            ← DeleteUserRequest (response is empty)
  shared.rs            ← types referenced by 2+ verbs (e.g. UserDto)
```

`shared.rs` is the **one allowed exception** to the no-helpers rule
inside a DTO folder, because data types genuinely *are* shared
between verbs. Keep it small — if `shared.rs` grows past ~100 lines,
the shared types deserve their own named file (`user_dto.rs`,
`pagination.rs`).

The corresponding `tools/<noun>/<verb>.rs` and the matching DTO
folder mirror each other one-to-one. That symmetry is the point:
opening `tools/user/create.rs` you know to look at
`dto/user/create.rs` for its types.

### Errors

One file per error domain. If `user` and `device` both have errors,
they get separate files. Never one mega `errors.rs`.

### Tests

One test file per source file, mirroring the tree.

```
src/user/create.rs
tests/user/create_test.rs
```

If `src/user/create.rs` has > 5 tests, split the test file by
scenario (`tests/user/create_validation_test.rs`,
`tests/user/create_persist_test.rs`).

### Frontend (React / TypeScript)

The same philosophy maps directly to the React app and extension UIs.
**One component per file; folder-of-components over file-of-many.**

```
src/features/user/
  index.ts             ← barrel: re-export only, no bodies
  UserList.tsx         ← one component, presentation + local wiring
  UserCard.tsx         ← one component
  useUserList.ts       ← one hook: data/state for the list
  user.api.ts          ← API calls to the node (one call per export)
  user.types.ts        ← view/DTO types (mirror the Rust `dto/user`)
  UserList.test.tsx    ← sibling test, mirrors the component
```

Rules specific to the frontend:

- **One component per `.tsx` file**, named in `PascalCase` matching the
  component. No "kitchen-sink" files exporting five components.
- **One hook per file**, named `use<Concept>.ts`. A hook is the
  frontend equivalent of a verb — name it by what it does
  (`useDebouncedSearch`, not `useHelpers`).
- **Separate data from markup.** When a component grows past ~150
  lines because of state/effects, extract the logic into a
  `use<Component>.ts` hook beside it; the `.tsx` keeps the markup.
- **`index.ts` is a barrel**, exactly like Rust's `mod.rs`: re-exports
  only, never component bodies. (See the naming table below.)
- **State/stores:** one slice/store per file, named by domain
  (`session.store.ts`, `workspace.store.ts`) — never `store.ts`.
- **API client:** one call per export, grouped by resource
  (`user.api.ts`), so it mirrors the backend `tools/user/<verb>.rs`
  and `dto/user/<verb>.rs` one-to-one. That cross-stack symmetry is
  the point: a verb has the same name in the tool, the DTO, and the
  client.
- **Extension UIs** (module-federation remotes, Web Components,
  iframes) follow the same per-feature folder shape so a remote looks
  native to anyone reading the tree.
- **Co-locate styles with the component.** This repo uses Tailwind +
  shadcn/ui, so styling lives in the JSX; avoid separate `.css` files
  unless a component genuinely needs them.

The 400-line hard limit and ~150-line typical target apply to
`.ts`/`.tsx` exactly as they do to `.rs`.

### Generated code

OpenAPI / protobuf / SQLx-emitted code is exempt from the 400-line
limit (we don't hand-edit it). Put it under `src/generated/` and
gate the rule on path: human files everywhere else stay under 400.

---

## 5. File-naming rules

| Never | Always |
|---|---|
| `utils.rs` / `utils.ts` | Name the concept: `retry.rs`, `token_cache.rs` |
| `helpers.rs` / `helpers.ts` | Name the concept: `slot_coerce.rs`, `url_builder.ts` |
| `common.rs` / `common.ts` | Move shared types to `*-spi`; name them |
| `misc.rs` / `support.rs` | Don't create. Trash drawers grow forever. |
| `mod.rs` with logic in it | `mod.rs` is a barrel. Body lives elsewhere. |
| `index.ts` with 30 exports of bodies | Same. Re-export only. |
| `<noun>.rs` doing every verb | `<noun>/<verb>.rs` per verb |
| `types.rs` / `models.rs` | Name them by what they model: `address.rs`, `principal.rs` |

If you cannot describe the file's job in one sentence without
"and" — it's two files.

---

## 6. The split heuristic

When you sit down to write a file or open one to edit, ask in order:

1. **One-sentence test.** Can I describe this file's job in one
   short sentence with no "and"? If no → it's two or more files.
2. **Blast-radius test.** If this file changes, what else might
   break? If the answer mentions things unrelated to this concept
   → it's mixed, split it.
3. **Filename test.** Would someone searching by filename find what
   they expect? `password.rs` → yes. `auth_stuff.rs` → no.
4. **Edit-locality test.** If two PRs both touch this file, do they
   touch the same lines or different concerns? If different concerns
   → split.

If you're about to write more than **~150 lines** in a new file,
pause and split first. Adding lines is cheap once the boundary is
right; refactoring after the fact is expensive.

---

## 7. When NOT to split

Discipline cuts both ways. Don't fragment for its own sake.

- A `Display` impl is fine in the same file as the type it formats.
- A small struct + its `Default` + its `new()` belong together.
- A handler and the single private helper it uses, where that helper
  is never called from elsewhere, may live together — until a second
  caller appears, at which point promote the helper.
- Trait impls for foreign types (`impl From<X> for Y`) live with the
  local type they produce (`Y`'s file).

Rule of thumb: split when there are **two distinct caller-visible
responsibilities**. Two private functions that always run together
in one verb's pipeline are not two responsibilities.

---

## 8. Migrating an existing oversized file

A practical sequence when you find an offender:

1. **Inventory.** List the distinct responsibilities in the file.
   If you write down "and" while listing, that's a split point.
2. **Create the folder.** `mv user.rs user/get.rs` (or the most
   accurate verb), then `mkdir -p user/` is already done.
3. **Add `mod.rs`** as a barrel that re-exports what was `pub`.
4. **Move verb by verb** in separate commits. Each commit:
   - moves one verb out into its own file
   - updates `mod.rs`
   - runs `cargo check`
   - runs the tests
5. **Co-locate tests** in the same sequence. Each verb's tests move
   into a sibling test file.
6. **Delete dead code** along the way. If a private helper is no
   longer called after the split, it was scaffolding — remove it.

Do not attempt a single mega-commit "split user.rs into the new
layout". Reviewers can't read it and bisect can't reach into it.

---

## 9. Enforcement

- **Author-time.** Every PR description states whether any file
  approaches 400 lines and why if so.
- **Review-time.** A reviewer who sees a file over 300 lines asks
  about a split.
- **CI** (planned): a `scripts/check-file-size.sh` that fails if any
  tracked `*.rs` / `*.ts` / `*.tsx` (excluding `src/generated/`) is
  over 400 lines.

Until the CI check lands, the rule is enforced by reviewers and by
this doc.

---

## 10. One-line summary

**One verb per file. Folder-of-verbs over file-of-nouns. ≤400 lines
hard, ~100 lines typical. Names are concepts, never shapes
(`utils`, `helpers`, `common`).**
