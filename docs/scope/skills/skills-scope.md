# Skills scope

Status: draft placeholder.

Define workspace-shared skills: reusable instructions, tool recipes, prompts,
checklists, and workflow guidance that AI agents can load when granted by a
workspace capability.

## Intent

Skills should be first-class workspace assets. A skill can be private to a user,
shared with a team, linked in a channel, installed by an extension, or made
available to a centralized cloud AI agent for remote workflow sessions.

## Questions

- What is the skill manifest shape?
- How are versions, authors, and compatibility tracked?
- How does a team approve a shared skill before agents can use it?
- Which capabilities let an agent read, execute, or update a skill?
- How are skill changes audited across edge and cloud nodes?

## Example flow

1. A developer creates a `coding-scope-writer` skill in a workspace.
2. The skill is shared with `team:engineering`.
3. A GitHub issue enters the inbox.
4. A coding workflow job asks the central AI agent to use the skill.
5. The AI drafts a scope doc and creates an approval inbox item.
6. On approval, outbox starts the remote coding workflow.

