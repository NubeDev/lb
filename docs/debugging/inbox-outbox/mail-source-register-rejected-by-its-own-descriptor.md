# `mail.source.register` was rejected by its own palette descriptor — twice, in two different ways

- Area: inbox-outbox (mail source) / host tools
- Status: fixed
- First seen: 2026-08-26 (first live registration of a mailbox on a running node)
- Resolved: 2026-08-26
- Session: ../../sessions/inbox-outbox/mail-source-session.md
- Regression coverage: `rust/crates/host/tests/mail_import_test.rs` exercises the SERVICE; the
  descriptor half is covered by the live drive recorded in the session doc (see "Lesson" — the
  gap this entry is really about is that the suite could not see it at all).

## Symptom

Two consecutive `400`s from `POST /mcp/call` on a node where every unit and integration test for
the feature was green:

```
$ mcp mail.source.register '{"source": {"id": "meter-data", …}}'
bad input: missing required arg: id — Workspace-unique id for this mailbox

$ mcp mail.source.register '{"id": "meter-data", …, "allowSenders": ["@example.com"]}'
bad input: arg `allowSenders` must be string
```

Neither error came from the verb. Both came from `tools::validate_args`, which checks a call's
arguments against the tool's **declared `input_schema`** before dispatch.

## Root cause

The verb and its descriptor disagreed, in opposite directions:

1. **Shape.** `call_mail_tool` was written to accept the source object *either* nested under a
   `source` key *or* spread flat, "because a roster form posts one shape and a curl example the
   other". The descriptor declared only the flat form, with `required: ["id", "host", "username",
   "secretPath"]`. So the nested body never reached the tolerant parse — the validator rejected it
   first, naming an argument the caller had in fact supplied one level down.

2. **Type.** `allowSenders` is `Vec<String>` on the record, but the descriptor declared it
   `"type": "string"` with the description "Comma-separated addresses or @domains", because that
   is the convenient thing for a palette form to render. `validate_args` does a shallow per-property
   type check, so the honest array shape — the one the record deserializes and the one every test
   used — was refused at the gate.

## Fix

Make the descriptor the single contract and the verb match it:

- **One shape, flat.** `source_arg()` is deleted; the arg object *is* the source. The tolerance was
  not tolerance — it was a second, unreachable shape.
- **`allowSenders` is declared `"type": "array"`** with `items: {type: string}` and an
  `x-lb.widget: "tags"` hint. The palette gets a hint about how to render it; the wire type is the
  record's type.

## Lesson

**A host-native verb's tests do not cross its descriptor.** Every test in the suite called
`mail_source_register(...)` directly, or `call_mail_tool` with a hand-built `Value` — neither goes
through `validate_args`, which only runs in `call_tool_at_depth`. So a descriptor that contradicts
its verb is invisible to a green suite and appears on the first real call, exactly like the
`gate_tool_for` alias misses this repo already catalogues (`media.upload_*`,
`series.retention.delete`) — same class, different chokepoint: those verbs were unreachable because
of the *cap* the gate demanded, these because of the *schema* the validator demanded.

Two habits fall out of it:

- When a verb declares a descriptor, **drive it once over `POST /mcp/call`** before calling the
  slice done. That single call is what found both of these.
- Treat the descriptor's `input_schema` as the API contract, not as UI decoration. If a form wants a
  friendlier shape than the API, the form converts — the schema does not lie about the type.
