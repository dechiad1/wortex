# Wortex
A tool to manage & orchestrate agents as system processes

## Conventions
- Use `task <command>` (Taskfile) for tests, builds, and other dev commands
- Use `bd` (beads) for issue tracking. See `AGENTS.md` for workflow details.

## Issue Tracking (beads)

This project uses [Beads](https://github.com/steveyegge/beads) for issue tracking. See `.beads/README.md` for basic commands.

**Important:** Do not store sensitive information (passwords, API keys) in issue descriptions.

### Workflow

Always work on feature branches, not main. Commit code and `.beads/` together so issue state stays in sync.

```bash
git checkout -b feature/my-feature
bd update <id> --status in_progress
# ... make changes ...
git add <files> .beads/
bd sync
git commit -m "..."
git push
```

### Session Completion

Work is NOT done until `git push` succeeds.

1. File issues for remaining work (`bd create`)
2. Run quality gates if code changed (`task test:unit`)
3. Close completed issues (`bd close <id>`)
4. Commit and push
5. Verify `git status` shows clean working tree
