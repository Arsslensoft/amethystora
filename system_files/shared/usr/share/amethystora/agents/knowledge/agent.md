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
  changes anything, nor does one that was not started through `amethystora-agent`, nor one whose own settings
  allow a tool that changes things without asking: remove that rule from the agent's settings.

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

## In the pill

- **What it reads**: while a read tool runs, the live pill says so by the tool's name (`ame agent doing off` hides it).
- **Ask right there**: `Super+/` opens a field in the pill's card; the chosen agent answers there with the five read tools and nothing else
  (`amethystora-agent ask-here`, for Claude Code only: the other agents open in a terminal). `Super+Shift+Space` is the next keyboard layout.
- **Questions**: a command or a setting you hand to the user (`run`, a `change` that asks) is put to them as a card in the pill, with the exact
  command, Allow and Not now (`ame agent ask-questions`). Only their click on the desktop answers; no file does. A root command's password is still
  asked by sudo in the terminal that opens after an Allow. Where there is no live pill the terminal asks, as it always did.
- **Routines**: `ame agent routines` keeps small things the desktop does by itself when something happens. You write and check one
  (`references/routines.md` of the amethystora skill); only the user keeps, runs or removes it. They are sealed in the Vault and run only while it is
  open, unless its background key is on (it needs a TPM). A routine's `ame` words are resolved first: a private alias of an `agent`, `security`, `vault`
  or `plugin` recipe is refused like the recipe.

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
