# The agent's own rules on Amethystora

Written for AI agents, so that the agent can explain to the user what it may do and why.

## Two levels

- **Agentic features** (level 1, `ame agent toggle on`): the agent, its skills, and the tools that only
  read: `search`, `read`, `state`, `logs`, `history`. They are pre-allowed: they never ask.
- **Let the agent make changes** (level 2, `ame agent tools on`, needs level 1): `change`, `software`, `run`
  and `undo`. Each asks the user in the agent's own prompt, every time.
- Both are off as the image ships, for every account; each account turns them on for itself, in Control's
  AI agent page or with those commands. The agent never turns its own levels on.
- The tools exist only while the agent was started through `amethystora-agent` or `ame agent` (or, for Cursor CLI and
  Antigravity CLI, while the features are on). An agent started with its own checks off gets no tool that
  changes anything.

## Root commands

`run` with `root: true` runs a command as root, in the mode an administrator chose for every account
(`ame agent root`, Control):

- `session`: one password, then until the agent ends, the screen locks, or an hour passes without a root
  command.
- `minutes`: one password, then five minutes for this agent's tool process only.
- `each`: a password for every command.
- `terminal`: the command opens in a terminal, where the user reads it and types their password for sudo.

Every root command is shown in a notification and written to the journal (`journalctl -t
amethystora-agent-root`). A command that asks questions always opens in a terminal, whatever the mode.

## The lock

An administrator sets the most any account may turn on (`ame agent lock act|know|off`), or one account's own
(`ame agent lock ACCOUNT LEVEL`), in `/etc/amethystora/agent.conf`. `know` means no changes. The agent may never
change its own lock or root mode.

## Never

The tools and the root helper refuse, whoever allows it, and the agent then names the command for the user to
run: what the main skill says never to touch; a security command that loosens protection or sends something away;
deleting from a backup or a snapshot; erasing a disk or answering ERASE; turning the AUR on; changing sudoers,
polkit, PAM or the agent's own lock; the Notes app's data; and any shell or wrapper that would run commands out
of sight. Security commands that turn protection on are handed over only when the user asked for them.

## Records

Every change is recorded with its state before and its way back: `history`, `ame agent history`. `undo` with
a record's id puts a setting or an app back. A command handed to the user has no undo of its own.

## What the user's provider sees

What the tools answer (passages, the machine's state, log lines) goes to the agent's provider, like anything
else the agent reads. The index holds nothing personal: not the user's files, not the journal.
