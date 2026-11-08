# The AI agent

An AI agent in a terminal that knows this system: which files are the image's and which are yours, how themes,
hotkeys and software work here, and what to ask you before it touches anything. It is off until you turn it on, in
[Control](control.md)'s **AI agent** page or with `ame agent toggle on`. Until then there is no hotkey, no menu entry,
no **Ask the agent** button, and no agent is told anything about this system.

### Opening it

`Super+Ctrl+Shift+A` opens the agent you chose, else the first one installed, else Claude Code. You can choose
Opencode, Codex, Antigravity CLI, Copilot CLI or Cursor CLI instead. It comes with a skill that explains this system
to it. Ask it things in plain language:

- "Make the dock icons smaller."
- "Why is my laptop fan always running?"
- "Set up a Python project with uv in ~/code/scraper."
- "Add a hotkey that opens Mission Center."
- "Make a top bar widget that shows the weather." See [Widgets](widgets.md).

### Before you say yes

It starts in its normal mode, so it asks before it runs a command or changes a file: read what it proposes before you
say yes. Opencode's own normal mode asks for almost nothing, so started from here it asks before a command, an edit
and a web fetch, unless your own `~/.config/opencode/opencode.json` sets a `permission`, which then wins.

A repository someone else wrote can tell an agent what to do as it opens. Before you start one in a folder you cloned,
read its `.claude/`, `.mcp.json` and `.vscode/tasks.json`, if it has them: each can name commands the agent or your
editor will run, or servers it will talk to, as you.

## What it can do

The agent comes with Amethystora's own tools, which it uses in place of reading whole pages and running commands one
by one. There are two levels, each turned on by you, for your account:

| Level | What the agent can do | How it asks |
| --- | --- | --- |
| **Agentic features** (`ame agent toggle on`) | Search this manual and the image's own reference of commands, services and settings; read this machine's state (updates, settings, security, apps, disks, backups, failed services, crashes) and its logs; see what it changed before | It does not ask: these only read |
| **Let the agent make changes** (`ame agent tools on`) | Set Amethystora's settings as Control does, install and remove apps (Flatpaks for you, Homebrew, containers), hand you commands to run, as root too, and undo what it changed | Before every change, in the agent; a command that asks questions opens in a terminal, where you read it and say yes |

The second level needs the first.

### What it may never do

Whatever you allow, the agent may never do anything about a duress finger or password, loosen a security setting or
turn on one that sends something away, delete from a backup or a snapshot, erase a disk, turn the AUR on, or change its
own lock or root mode. It names such a command, and you run it yourself. It runs a security command only when you ask, and
only one that turns protection on.

### Where the tools work

The tools only work while the agent was started from here (`Super+Ctrl+Shift+A`, the menu or
`amethystora-agent`), except in Cursor CLI and Antigravity CLI, whose own settings hold them while the
features are on. An agent started with its own checks turned off (such as `--dangerously-skip-permissions`)
gets no tool that changes anything.

## Turning it on and off

The agent features are off on a new machine and for every new account; an update turned them off for every account
that had them on, and said so once. Each account turns them on for itself:

```bash
ame agent toggle on    # the menu entries, the hotkey, the Ask the agent buttons, and the skill
ame agent toggle off   # takes them away again, and changes with them
```

An agent you installed stays installed either way.

## Diagnose a problem

**Diagnose a problem** in the Amethystora menu (`Super+Alt+Space`) hands the agent something that went wrong: a
crash, a failed service, or your own description ("Wi-Fi drops after suspend"). It investigates from the logs and
crash reports, changes nothing, and comes back with the cause and a proposed fix.

```bash
amethystora-agent diagnose                       # look for anything that failed recently
amethystora-agent diagnose "wifi drops after suspend"
amethystora-agent diagnose NetworkManager.service
amethystora-agent diagnose 4242                  # a process ID
```

### From the apps

The apps hand it what went wrong where you see it: **Ask the agent why** in **Updates** when an update did not finish,
**Ask the agent** in **Security** beside anything that needs your attention, and in **Logs** beside a crash or
something a service wrote. They pass on only the service, the process number or the report's own words, and the agent
reads the rest itself. While the agent features are off, the buttons are not there.

### The health check

`ame system doctor` is the health check the agent starts from: failed services, updates, disk space,
memory, crashes, the clock, the driver, backups and pools of disks, each finding with the guide that
fixes it. It reads only, and you can run it yourself; `ame system doctor ID` checks one thing again after
a fix.

## Commands

| Command | Does |
| --- | --- |
| `amethystora-agent` | Open the agent (the same as `Super+Ctrl+Shift+A`); also `ame agent` |
| `amethystora-agent ask "<question>"` | Open it with a first question; also `ame agent ask` |
| `amethystora-agent diagnose [...]` | Find out why something broke, changing nothing; also `ame agent diagnose` |
| `amethystora-agent use <agent>` | Choose the agent: `claude`, `opencode`, `codex`, `agy`, `copilot` or `cursor-agent`; also `ame agent use` |
| `amethystora-agent install [<agent>]` | Install the chosen agent ahead of time, or the one named without choosing it; also `ame agent install` |
| `amethystora-agent list` | Each agent, and whether it is installed; also `ame agent list` |
| `amethystora-agent status` | Whether the agent features are on for your account, and which agent; also `ame agent status` |
| `ame agent toggle on` | Turn the agent features on for your account, or `off` again |
| `ame agent tools on` | Let the agent make changes, asking each time, or `off` again |
| `ame agent history [ID]` | What the agent changed, or one change and how it ended |
| `ame agent lock`, `ame agent root` | What your account may turn on, the lock and the root mode, changing nothing; also `amethystora-agent policy` |
| `amethystora-agent search "<question>"` | Search the manual and the image's reference as the agent does; `-k N` gives N passages instead of 4 |
| `amethystora-agent read ID` | Read a whole passage a search found, by its id |
| `amethystora-agent state TOPIC` | This machine's state as the agent reads it: `overview`, `health`, `settings`, `security`, `security-settings`, `app-permissions`, `software`, `updates`, `backups`, `disks`, `setup-diff`, `units` or `crashes` |

`ame agent search`, `read` and `state` are the same three. `toggle` and `tools` without `on` or `off` switch to the other. The search, `read`, `state` and the history work
only while the agent features are on. [Control](control.md)'s **AI agent** page has the same switches and
choice, and installs the chosen agent.

## Installing and updating

The agents are not part of the image: they release far more often than it does. The first time you open one, it
offers to install it into your home with its maker's own installer. From then on it keeps itself up to date, and you
can update it yourself: `claude update`, `opencode upgrade`,
`codex update`, `agy update`, `copilot update` or `cursor-agent update`. Claude Code is installed on its stable
channel, a release about a week old. Because it lives in your home, the same copy also works inside
any toolbox or distrobox.

### Antigravity CLI

Antigravity CLI (`agy`) is Google's agent, in place of Gemini CLI. A Gemini CLI you already chose and
installed still opens, and `ame update` upgrades it with the rest of your Homebrew apps; to move,
choose `agy`. Antigravity's installer also adds `~/.local/bin` to your shell's `PATH` in your shell's
startup files.

## Privacy

No agent does anything until you sign in to it, and what you send it goes to the provider you
signed in to. So does what its tools answer: the search runs on this machine, but the passages,
the machine's state and the log lines the agent reads go to that provider like anything else it reads.
Nothing personal is in the search's index: not your files, not the journal. **The agent never gets into the [Vault](vault.md)**:
its tools refuse every command and file of it, and if you ask it for a password it tells you to open the Vault yourself.
