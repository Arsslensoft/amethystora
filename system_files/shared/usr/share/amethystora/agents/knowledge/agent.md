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

## Several accounts of one agent

A profile is another login of the same agent (a work account of Claude Code beside the user's own), with its own history and settings, in
`~/.local/share/amethystora/agent-profiles/NAME/`; `default` is the home's own folders. The person sets it up; you never do:

- `ame agent profile add NAME`, `ame agent profile login NAME [AGENT]` (a terminal), `ame agent profile use NAME [FOLDER]` (a folder and everything
  under it uses it), `ame agent profile terminal NAME [FOLDER]`, `ame agent profile remove NAME` and `ame agent profile hook on|off`.
- You may run `ame agent profile list`, `ame agent profile which [FOLDER]` (which account an agent started in that folder gets) and
  `ame agent profile running` (the sessions running now). Everything else of it is refused to you.
- A launch gets `--profile NAME`, else the terminal's pin, else the folder's profile, else `default`. The folder's rule is the person's own file;
  nothing is read from a repository, so a project cannot pick one. A running agent never changes account, and you never set `CLAUDE_CONFIG_DIR`,
  `CODEX_HOME` or `AMETHYSTORA_AGENT_PROFILE_PIN`: the tools refuse it.
- Copilot CLI has no profiles (its login is in the keyring). A profile does not share one subscription between people. Nothing of profiles is in the
  setup file.

## An agent held to its project

A sandbox lets the agent write in its project, its own folder and the build caches, and hides the person's keys and logins from it; there is no
sudo, no session bus and no keyring inside. The person turns it on for a folder, and nothing is read from the repository. You may run
`ame agent sandbox show [FOLDER]` (what an agent started there is given) and `ame agent sandbox list`. The rest is the person's:

- `ame agent sandbox use [FOLDER] [open|closed]`, `ame agent sandbox off [FOLDER]`, `ame agent sandbox allow FOLDER PERMISSION` and
  `ame agent sandbox disallow FOLDER PERMISSION`, where a permission is `write=PATH`, `show=PATH`, `network=HOST`, `ssh-agent` or `gpu`, each asked for in a
  terminal after it says what it gives away; `ame agent sandbox network FOLDER list|open` holds the sandbox's network to a list of hosts over
  HTTPS (the agent's maker, the package registries, the code hosts, and each `network=HOST`), through a proxy, or gives the machine's back; `ame agent sandbox run -- COMMAND` holds any command the same way; `ame agent sandbox always on|off`
  is an administrator's.
- It is not a virtual machine, does not make what you wrote safe to run (a changed build script runs as the person when they run it), and
  does not hide the project's own `.env`. Codex must have run once outside a sandbox; Copilot CLI needs its token in `COPILOT_GITHUB_TOKEN` or `GH_TOKEN`.
- While the network is the machine's, a sandboxed agent still reaches what listens on the machine (`localhost`, X11) and can send what it reads;
  held to a list, a program that ignores `HTTPS_PROXY` (git over SSH) reaches nothing, and `localhost` outside the sandbox is out of reach.
- Inside one, `AMETHYSTORA_AGENT_SANDBOX` is set. A read-only file system outside the project, a missing login or key, no session bus and no sudo are
  the sandbox: say so, name the permission, and look for no way round (`references/sandbox.md` of the amethystora skill).

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
