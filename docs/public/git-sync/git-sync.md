# Git-sync (public)

TODO — fill on ship. Promotes from `../../scope/git-sync/autocommit-scope.md` once the
periodic auto-commit-and-push lands.

Will document: the ported `lb-gh` crate, the `git.*` MCP verbs (`git.commit_push`, `git.status`)
with their capabilities, the `git-sync` job kind, the `Action::McpTool` reminder that schedules it,
and the `systemd` unit that keeps the node (and thus the reminder reactor) alive.
