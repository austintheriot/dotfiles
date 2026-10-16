---
name: agent-notify
description: Use when a task will run long, loops on a wait (CI, deploys, polling), or reaches a result the user must see. Tells you when to call `agent-notify send` and `agent-notify quiet`, and when to stay silent.
---

# agent-notify

The user gets a desktop notification each time you stop. The hooks send it. You do not need to do anything for an ordinary turn.

Use the CLI only to make a notification more useful, or to stop noise.

## `agent-notify send`

```bash
agent-notify send --kind attention --body "CI failed on lint: 3 errors in src/api.ts"
agent-notify send --kind info --body "Benchmarks done: p99 down 18%"
```

- `attention`: the user must act (a decision, a failure, a review). It shows as an amber card and stays until the user clicks it.
- `info`: a result the user wants but does not have to act on. It fades.
- The body is one short sentence. Name what finished and what the user must do. No preamble, no markdown.
- Send at most once per result. Do not send progress updates.

## `agent-notify quiet`

Before a loop that stops and starts many times (a CI watch, a poll, a retry loop), turn off the per-stop notification:

```bash
agent-notify quiet --until next-send --max-minutes 180
```

Then call `agent-notify send` once, with the outcome. That send ends the quiet period. `--until next-prompt` ends it when the user types instead. `agent-notify quiet --off` ends it now.

`quiet` needs a tmux pane that a SessionStart hook tagged. Outside tmux, it exits 2. When that happens, continue without it.

## Do not

- Do not send when the turn ends normally. The Stop hook already notifies.
- Do not send a permission request. The Notification hook already does.
- Do not put secrets, tokens, or file contents in `--body`. The text goes to the OS notification center and a log file.
