# wortex - Worktree + Tmux Manager CLI

Rust CLI that manages git worktrees paired with tmux windows. Tracks state globally, handles lifecycle.

## Commands

```
wortex init
wortex new <branch> --prompt <prompt> [--agent <agent>] [--exit-kill[=<codes>]] [--remote <remote>] [--base <branch>]
wortex new <branch> --cmd <cmd> [--exit-kill[=<codes>]] [--remote <remote>] [--base <branch>]
wortex list [--json]
wortex switch <branch>
wortex kill <branch> [--keep-worktree]
wortex cleanup [--dry-run]
wortex status
```
### `wortex init`
**Behavior:**
1. Check if the `~/.wortex` directory exists
2. Create it if it does not exist (directory only, no state.json yet)

#### Global Checks
**Important:** All subsequent commands should check for the presence of `~/.wortex`, failing with "Run `wortex init` first" if not present.

### `wortex new`

Creates worktree + tmux window + runs command.

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--prompt` | yes* | - | Prompt passed to claude |
| `--cmd` | yes* | - | Arbitrary command (mutually exclusive with --prompt) |
| `--agent` | no | - | Agent identifier passed to claude as `--agent` |
| `--exit-kill` | no | - | Kill pane on exit. No value = exit 0. `any` = any code. `0,1` = specific codes |
| `--remote` | no | `origin` | Git remote |
| `--base` | no | `main` | Base branch to create worktree from |

*One of `--prompt` or `--cmd` required.

**Behavior:**
1. Validate running inside tmux (check `$TMUX`), fail if not
2. Validate running in git repo root (not a worktree), fail if not
3. Validate remote exists, fail if not
4. Derive project prefix from remote URL (e.g. `git@github.com:user/myproject.git` → `myproject`, `git@github.com:user/my-project.git` → `mp`, `git@github.com:user/my_project.git` → `mp`)
5. Fail if branch already exists locally & print the reason for failure
6. Fail if worktree directory already exists & print the reason for failure
7. `git fetch <remote>`
8. `git worktree add ../<prefix>-<branch> -b <branch> <remote>/<base>`
9. Create state entry under ~/.wortex
10. Run wrapper: `tmux new-window -n <branch> -c <worktree-path> "wortex __run <id>"`

### `wortex __run <id>` (hidden)

Internal command executed inside tmux window.

1. Load state entry by id
2. Build command:
   - If `cmd`: run it directly
   - If `prompt` + `agent`: `claude --agent <agent> "<prompt>"`
   - If `prompt` only: `claude "<prompt>"`
3. Spawn command as child process, wait for completion, capture exit code
4. If `exit_kill` matches code: remove entry from state, kill own tmux window
5. Else: update state with exit code (pane stays open via remain-on-exit set during window creation)
6. Exit with captured code

**Implementation note:** Use `std::process::Command` to spawn and `.wait()` to get exit status. The remain-on-exit option is set when the window is created, so the pane stays open after `wortex __run` exits.

### `wortex list`

Show tracked worktrees.

```
BRANCH          TMUX                PATH                             STATUS    EXIT
feature-login   dev:feature-login   ~/projects/myproj-feature-login  running   -
feature-api     dev:feature-api     ~/projects/myproj-feature-api    exited    0

Tip: Use `wortex switch <branch>` or `tmux select-window -t <session>:<window>`
```

`--json` outputs array of state entries.

### `wortex switch <branch>`

`tmux select-window -t <current-session>:<branch>`

Fail if not tracked or window doesn't exist.

### `wortex kill <branch>`

1. Kill tmux window if exists
2. Remove git worktree (unless `--keep-worktree`)
3. Delete local branch (local only, not remote)
4. Remove from state

### `wortex cleanup`

Find stale entries:
- Worktree path doesn't exist
- Tmux window doesn't exist

Remove from state. With `--dry-run`, just print what would be removed.

### `wortex status`

Run `git status -s` in each tracked worktree, grouped by branch.

## State

**Location:** `~/.wortex/state.json`

**File Locking:** Use `~/.wortex/state.lock` with `fs2` crate for advisory file locking. Acquire exclusive lock before read-modify-write operations. Lock is released when file handle is dropped.

```rust
use fs2::FileExt;

fn with_state_lock<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let lock_path = dirs::home_dir().unwrap().join(".wortex/state.lock");
    let lock_file = File::create(&lock_path)?;
    lock_file.lock_exclusive()?;  // blocks until lock acquired
    let result = f();
    // lock released on drop
    result
}

struct State {
    version: u32,
    entries: Vec<Entry>,
}

struct Entry {
    id: Uuid,
    project: String,       // derived prefix
    branch: String,
    path: PathBuf,
    tmux_session: String,
    tmux_window: String,   // same as branch
    command: Command,
    exit_kill: Option<ExitKill>,
    exit_code: Option<i32>,
    created_at: DateTime<Utc>,
}

enum Command {
    Claude { prompt: String, agent: Option<String> },
    Raw { cmd: String },
}

enum ExitKill {
    Codes(Vec<i32>),
    Any,
}
```

## Validation Rules

| Condition | Error |
|-----------|-------|
| ~/.wortex missing | "Run `wortex init` first" |
| Not in tmux | "Must run inside tmux session" |
| Not git repo | "Not a git repository" |
| Inside worktree (not main) | "Must run from main repo, not a worktree" |
| Remote doesn't exist | "Remote '<remote>' not found" |
| Branch exists | "Branch '<branch>' already exists" |
| Worktree dir exists | "Directory '<path>' already exists" |
| Neither --prompt nor --cmd | "Must specify --prompt or --cmd" |
| Both --prompt and --cmd | "--prompt and --cmd are mutually exclusive" |

## Detecting Main Repo vs Worktree

```bash
git rev-parse --git-common-dir  # returns .git for main, ../.git for worktree
git rev-parse --git-dir         # returns .git for main, .git/worktrees/<name> for worktree
```

If they differ, we're in a worktree.

## Tmux Operations

**Get current session name:**
```bash
tmux display-message -p '#S'
```

**Create window with remain-on-exit:**
```bash
tmux new-window -n <branch> -c <worktree-path> "wortex __run <id>"
tmux set-option -t <session>:<branch> remain-on-exit on
```

**Check if window exists:**
```bash
tmux list-windows -t <session> -F '#W' | grep -q '^<window>$'
```

**Kill window:**
```bash
tmux kill-window -t <session>:<window>
```

## Deriving Project Prefix

Always attempt acronym extraction. Split on `-` or `_`, take first char of each part.

```rust
fn get_project_prefix(remote: &str) -> Result<String> {
    // 1. git remote get-url <remote>
    // 2. Parse: git@github.com:user/project.git -> project
    //    Parse: https://github.com/user/project.git -> project
    // 3. Strip .git suffix
    // 4. Generate acronym from name
}

fn to_acronym(name: &str) -> String {
    // Split on '-' or '_'
    // If single part (no separators): return name as-is
    // If multiple parts: take first char of each part, lowercase
    //
    // Examples:
    //   "myproject"    -> "myproject" (no separators)
    //   "my-project"   -> "mp"
    //   "my_project"   -> "mp"
    //   "my-cool-app"  -> "mca"
    //   "foo-bar_baz"  -> "fbb" (split on both)
}
```

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
dirs = "5"
fs2 = "0.4"              # file locking

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

## Project Structure

```
src/
├── main.rs
├── cli.rs           # clap definitions
├── commands/
│   ├── mod.rs
│   ├── init.rs
│   ├── new.rs
│   ├── run.rs       # __run
│   ├── list.rs
│   ├── switch.rs
│   ├── kill.rs
│   ├── cleanup.rs
│   └── status.rs
├── state.rs         # State + Entry + load/save + locking
├── git.rs           # worktree ops, remote parsing, prefix derivation
├── tmux.rs          # window ops, session detection
└── error.rs
```

## Testing

Integration tests use real tmux + git. Test helper creates temp repo.

```rust
struct TestEnv {
    dir: TempDir,
    repo: PathBuf,
    state_file: PathBuf,
}

impl TestEnv {
    fn new() -> Self;                              // creates temp git repo, sets env vars
    fn wortex(&self, args: &[&str]) -> Command;    // runs wortex with test env
    fn tmux_window_exists(&self, name: &str) -> bool;
    fn wait_for_exit(&self, name: &str, timeout: Duration);
    fn state(&self) -> State;
}
```

### Required Tests

1. **Happy path:** `wortex new` with `--cmd "echo test"`, verify window exists, exit code 0
2. **Exit kill:** `--exit-kill` auto-removes on success
3. **Exit kill specific:** `--exit-kill=1` only kills on code 1
4. **No exit kill:** pane stays open after exit
5. **Validation:** fails outside tmux, fails in worktree, fails if branch exists
6. **Kill:** removes window + worktree + state
7. **Cleanup:** removes stale entries

### Test Execution Note

Tests require tmux server running. CI needs:
```bash
tmux new-session -d -s test
# run tests
tmux kill-server
```
