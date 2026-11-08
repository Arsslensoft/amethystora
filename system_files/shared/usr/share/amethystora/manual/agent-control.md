# Keeping the agent in check

How root commands are asked for, what an administrator can cap, and how to see and undo what the [agent](ai-agent.md)
changed.

## Root commands

When you let the agent make changes, a command it needs as root asks you first in the agent, then as an administrator
chose in [Control](control.md)'s **AI agent** page, for every account:

| Mode | What your password gives |
| --- | --- |
| **Once per agent session** (`session`, the default) | One password; then root commands run until the agent ends, the screen locks, or an hour passes without one |
| **For five minutes** (`minutes`) | One password, then five minutes for that agent alone |
| **Every time** (`each`) | A password for every command |
| **In a terminal** (`terminal`) | Each command opens in a terminal, where you read it and type your password for `sudo` |

`ame agent root MODE` changes it. Every root command shows a notification and is written to the journal
(`journalctl -t amethystora-agent-root`). A command that asks questions always opens in a terminal.

## The lock

An administrator can cap what any account may turn on, or turn the agent off for one account (a child's, say), in
Control or with `ame agent lock`:

```bash
ame agent lock act|know|off       # act: both levels; know: the agent and its read-only tools; off: nothing
ame agent lock ACCOUNT know       # one account's own
ame agent lock ACCOUNT default    # back to everyone's
```

The lock governs Amethystora's agent features; it is not a sandbox, and for a child's account Parental Controls' app
filter is the stronger tool. Both settings, the lock and the root mode, are in `/etc/amethystora/agent.conf`.

## History and undo

Every change the agent's tools make is recorded, with what it was before and how to undo it: `ame agent history`,
or **What the agent changed** in Control. Undo one yourself with `ame agent undo ID` (or the pill's
[Undo](agent-desktop.md#its-changes-in-the-live-pill)), or ask the agent to. A command it handed you has no undo of its
own: the agent can tell you what would reverse it.
