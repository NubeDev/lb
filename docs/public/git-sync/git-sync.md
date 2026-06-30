# Git-sync (public)

TODO — fill on ship. Promotes from `../../scope/git-sync/autocommit-scope.md` once the
periodic auto-commit-and-push lands.

Will document: the ported `lb-gh` crate, the `git.*` MCP verbs (`git.commit_push`, `git.status`)
with their capabilities, and the `Action::McpTool` reminder (riding the existing `reminder-fire`
job) that schedules it —
and the `systemd` unit that keeps the node (and thus the reminder reactor) alive.
