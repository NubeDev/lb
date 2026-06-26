# Document store scope

Status: draft placeholder.

Define the shared document store for workspace docs, scope docs, specs, PDFs,
generated outputs, comments, attachments, and review artifacts.

## Intent

Documents should be durable workspace assets built on the core store, files,
tags, capabilities, inbox/outbox, jobs, and channel model. A document can be
private, shared with a team, linked to a channel, attached to an inbox item, or
used as context by local and centralized AI agents.

## Requirements

- Workspace-scoped ownership and access checks.
- User, team, and channel sharing.
- Tags for search and workflow routing.
- Version snapshots for AI-generated and human-edited docs.
- Review and approval states that can create inbox items.
- Outbox integration for publishing, sending, syncing, or external comments.
- Job integration for long-running document workflows such as scope drafting,
  review packs, summaries, exports, and release notes.

## Example flow

1. A GitHub issue arrives in the workspace inbox.
2. A workflow extension asks the central AI agent to draft a scope doc.
3. The document store saves the draft and shares it with `team:engineering`.
4. The workflow creates approval inbox items for reviewers.
5. Once approved, outbox comments on GitHub and starts the coding job.
6. The final PR summary, review notes, and generated docs are saved back into
   the document store.

