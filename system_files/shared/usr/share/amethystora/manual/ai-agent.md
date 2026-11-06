# The AI agent

The AI agent is off until you turn it on: in [Control](control.md)'s **AI agent** page, or with
`ame agent toggle on`. Until then nothing on this page is there: no hotkey, no menu entries, no
**Ask the agent** buttons, and no agent is told anything about this system.

Once it is on, `Super+Ctrl+Shift+A` opens an AI agent in a terminal: the one you chose, else the first one already installed, else Claude Code. You can choose Opencode, Codex,
Antigravity CLI, Copilot CLI or Cursor CLI if you prefer. It comes with a skill that explains this system
to it, so it knows which files belong to the image and which are yours to change, how themes and
hotkeys work, how software is installed on an image-based system, and what to ask you before it
touches anything.

Ask it things in plain language:

- "Make the dock icons smaller."
- "Why is my laptop fan always running?"
- "Set up a Python project with uv in ~/code/scraper."
- "Add a hotkey that opens Mission Center."
- "Make a top bar widget that shows the weather." See [Widgets](widgets.md).

It starts in its normal mode, so it asks before it runs a command or changes a file. Read what it
proposes before you say yes. Opencode's own normal mode asks for almost nothing, so started from here it
asks before a command, an edit and a web fetch, unless your own `~/.config/opencode/opencode.json` sets a
`permission`, which then wins.

A repository someone else wrote can tell an agent what to do as it opens: before you start one in a
folder you cloned, read its `.claude/`, `.mcp.json` and `.vscode/tasks.json`, if it has them. Each can
name commands the agent or your editor will run, or servers it will talk to, as you.

## What it can do

The agent comes with tools of Amethystora's own, which it uses in place of reading whole pages and
running commands one by one. There are two levels, each turned on by you, for your account:

| Level | What the agent can do | How it asks |
| --- | --- | --- |
| **Agentic features** (`ame agent toggle on`) | Search this manual and the image's own reference of commands, services and settings; read this machine's state (updates, settings, security, apps, disks, backups, failed services, crashes) and its logs; see what it changed before | It does not ask: these only read |
| **Let the agent make changes** (`ame agent tools on`) | Set Amethystora's settings as Control does, install and remove apps (Flatpaks for you, Homebrew, containers), hand you commands to run, as root too, and undo what it changed | Before every change, in the agent; a command that asks questions opens in a terminal, where you read it and say yes |

The second level needs the first. Some things the agent may never do, whatever you allow: anything about
a duress finger or password, loosening a security setting or turning on one that sends something away,
deleting from a backup or a snapshot, erasing a disk, turning the AUR on, or changing its own lock or root
mode. It names such a command, and you run it yourself. It runs a security command only when you ask, and
only one that turns protection on.

The tools only work while the agent was started from here (`Super+Ctrl+Shift+A`, the menu or
`amethystora-agent`), except in Cursor CLI and Antigravity CLI, whose own settings hold them while the
features are on. An agent started with its own checks turned off (such as `--dangerously-skip-permissions`)
gets no tool that changes anything.

## Turning it on and off

The agent features are off on a new machine and for every new account, and an update turned them
off for every account that had them on, which a notification said once. Each account turns them on
for itself: `ame agent toggle on` adds the menu entries, the hotkey and the **Ask the agent** buttons,
and links the skill; `ame agent toggle off` takes them away again, and changes with them. An agent you
installed stays installed either way.

## Diagnose a problem

**Diagnose a problem** in the Amethystora menu (`Super+Alt+Space`) hands the agent something that
went wrong: a crash, a failed service, or your own description ("Wi-Fi drops after suspend"). It
investigates from the logs and crash reports, changes nothing, and comes back with the cause and a
proposed fix.

```bash
amethystora-agent diagnose                       # look for anything that failed recently
amethystora-agent diagnose "wifi drops after suspend"
amethystora-agent diagnose NetworkManager.service
amethystora-agent diagnose 4242                  # a process ID
```

The apps hand it what went wrong where you see it go wrong: **Ask the agent why** in **Updates** when
an update did not finish, **Ask the agent** in **Security** beside anything that needs your attention,
and **Ask the agent** in **Logs** beside a crash or something a service wrote. They pass on only the
service, the process number or the report's own words, and the agent reads the rest itself. While the
agent features are off, the buttons are not there.

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

## Root commands

When you let the agent make changes, a command it needs to run as root asks you first in the agent, and
then as an administrator chose in [Control](control.md)'s **AI agent** page, for every account:

| Mode | What your password gives |
| --- | --- |
| **Once per agent session** (`session`, the default) | One password; then root commands run until the agent ends, the screen locks, or an hour passes without one |
| **For five minutes** (`minutes`) | One password, then five minutes for that agent alone |
| **Every time** (`each`) | A password for every command |
| **In a terminal** (`terminal`) | Each command opens in a terminal, where you read it and type your password for `sudo` |

`ame agent root MODE` changes it. Every root command shows a notification and is written to the journal
(`journalctl -t amethystora-agent-root`). A command that asks questions always opens in a terminal.

## The lock

An administrator can cap what any account may turn on, in Control or with `ame agent lock act|know|off`:
`act` allows both levels, `know` only the agent and its read-only tools, `off` nothing. One account can
have its own, `ame agent lock ACCOUNT know`, and `ame agent lock ACCOUNT default` puts it back. The lock
governs Amethystora's agent features; it is not a sandbox, and for a child's account Parental Controls'
app filter is the stronger tool. Both settings are in `/etc/amethystora/agent.conf`.

## History and undo

Every change the agent's tools make is recorded, with what it was before and how to undo it:
`ame agent history`, or **What the agent changed** in Control. Undo one yourself with `ame agent undo ID`,
or ask the agent to. A command it handed you has no undo of its own: the agent can tell you what would
reverse it.

## Its changes in the live pill

While the agent may make changes and runs, the live pill in the middle of the top bar says so as soon as
it changes something: **Agent · changing settings**, **Agent asks for root** while it waits for your
password or your yes in a terminal, and **Agent made 3 changes** for ten minutes after it ends. Click the
pill to see each change: what it was, the command or the app, and when, with its own **Undo**. **Undo
all** takes them back, the latest first. **Stop the agent** ends its tools now (`ame agent stop` does the
same); a change already under way, such as an install, finishes, and some agents start their tools again, so
close the agent itself to be sure it changes nothing more. **History** opens the whole record.

Undo is your own yes, and goes through the agent's own tools: an undo they refuse (a setting no longer
offered, a record whose way back does not match what it says it did) is refused, and the row says why. The
agent's tools will not turn this list off: `ame agent pill off` does, and is yours to run.

## What it is doing

While the agent's tools read, the pill says which one in a few words: **Searching the manual**, **Checking
the updates**, **Reading the logs**, with its mark breathing, and a tick when the step is done. Click it for
the last few steps, each Done or Now, and **Stop the agent**. It shows only the name of the tool, never what
you asked or what it read, and it is gone a few seconds after the last step. `ame agent doing off` turns it
off.

## Asking in the pill

Press `Super+/` (`Super+Shift+Space` is the next keyboard layout) and the pill's card opens with a field.
Type your question and press Enter: your agent answers there, with no terminal. It looks at this computer
with Amethystora's tools that only read (the manual, the machine's state, the log), and nothing else: it
cannot change anything and cannot read your files. The card says whose answer it is, and that your question
left this computer, for the maker of the agent you chose. The answer is shown as plain text, kept for ten
minutes at most, and nothing of it is written to a file by Amethystora.

It works with Claude Code. The other agents have no way to ask a question that changes nothing which this
image has checked, so for them the card offers to open the agent in a terminal instead. `ame agent ask-here
off` turns the key off. If the live pill is off there is nowhere to answer, and the key opens the agent in a
terminal.

## Questions in the pill

When the agent's tools ask to run a command for you, a setting that asks something, or a command as root in a
terminal, the pill says **Agent asks you** and its card shows the exact command, the agent's reason in its
own words, and **Allow** and **Not now**. Allow opens the terminal that runs it and sends what it prints back
to the agent; Not now tells the agent no. A command too long to read in a card is asked in the terminal,
where it is printed whole, and a root command still asks for your password there, from sudo.

Your yes is a click in the desktop, answered over the session bus: no file the agent could write stands for
it, and the command you read is checked against what the tools wrote. A question that nobody answered (the screen
locked, the features went off, an hour passed) is told to the agent as not answered, never as a no. `ame agent ask-questions off` makes them ask in a terminal again. Installing an app or changing a setting
with the agent's own tools is still asked for in the agent itself.

## Asking about a picture or a file

Off until you turn it on with `ame agent ask-about on`. Then drop a picture or a text file on the pill's
**Ask the agent**, or copy a picture (a screenshot of a region, say) and press `Super+Shift+A` with no text
selected. A card shows the file and says what goes and to whom: "This picture goes to the maker of Claude
Code. What is on it goes with it." Nothing is read past the first bytes of a text file, copied or sent until you
press **Send and ask**; a picture is copied for the moment the question takes into a folder only you can
read, and taken away again. It is refused while your screen is shared or recorded.

## Little routines

A routine is something done by itself when something happens: "when I join the office Wi-Fi, open Files and
mute". One readable file names **one trigger**, a time (with days), a network, an app opening, closing or going
fullscreen, the screen unlocking or locking, the battery below a level or unplugged or plugged in, or your
phone coming near or going away, and up to eight **actions**: an app to open by its launcher's id, or an `ame`
command as words, with an optional `undo` command that puts it back:

```json
{"name": "Office morning",
 "when": {"network": "Office Wi-Fi"},
 "do": [{"open": "org.gnome.Nautilus.desktop"},
        {"ame": ["desktop", "volume", "mute"], "undo": ["desktop", "volume", "unmute"]}]}
```

Ask the agent to write one; it checks it with `ame agent routines check FILE`. **Keeping it is yours**:
`ame agent routines add FILE` shows what it does in words and asks in a terminal, and the agent's tools will
not run it. A routine runs as you, never as root, never a shell line, never an `ame security`, `vault`,
`agent` or `plugin` command, and never anything the agent itself may not run; a file with something that
looks like a password or a token in it is refused. `list`, `show`, `pause`, `resume`, `remove`, `run` and
`undo` manage them.

Each run shows in the pill once: **Office morning ran**, what it did, **Pause this routine** and **Undo** for
a minute (Undo runs the way back the file gave, and closes the apps the routine opened that were not open
before). A routine that fails says so once and pauses itself until you start it again. The same trigger twice in
ten seconds runs it once. While the screen is locked this extension is off, so a time that falls then is missed;
a routine for the screen locking has to say `"while_locked": true`.

What a routine names tells a stranger about you, so they are all **sealed in the [Vault](vault.md)**, under its
background key. **A routine runs only while the Vault is open, unless that key is on** (`ame vault methods
background on`, which needs a TPM): locking the screen closes the Vault, and without the key the routines stop
until you open it again. Nothing waits for you and nothing is queued: what happens meanwhile is missed, a notice
says once that your routines wait for the Vault (with **Open Vault**), and a routine that finds the Vault closed
when its trigger fires says so once. While the Vault's own window is open the desktop looks again every
fifteen seconds and starts listening as soon as it can. `ame agent routines add` says all this before it keeps one.

Routines are part of the agentic features: they run only while those are on (`ame agent routines off` stops them
too), and with none made nothing is made, watched or asked: no folder, no listening, and no call to the Vault.
Only the sources a routine names are listened for.

## Asking about a selection

Select text in any window, an error in a terminal, a paragraph on a web page, a line in a log, and press
`Super+Shift+A`. A card shows the start of what you selected, with a field for your question (leave it
empty to have it explained), **Cancel** and **Ask**. Ask opens the agent in a terminal with your question,
and with the text as data: the agent is told it was written by someone else and never to follow what it
says, since selected text can carry instructions written to steer an agent. Up to 4,000 characters of it
are passed on, each line at most 600; nothing is read before you press the keys, and nothing is kept.

It works only while the agentic features are on. `ame agent selection off` turns the key off.

## Notices that offer the agent

While the agentic features are on, the image's own notices of something that failed, a security scan
that could not run and a device the driver check found without a driver, have an **Ask the agent**
button. It opens the agent on the failed service or the check's finding, by its name, never on the
notice's words. Other apps' notifications never get the button: anyone can raise a notification, and
its words could be written to steer the agent.

## Installing and updating

The agents are not part of the image, because they release far more often than it does. The first
time you open one, it offers to install it into your home with its maker's own installer. From then
on it keeps itself up to date, and you can update it yourself: `claude update`, `opencode upgrade`,
`codex update`, `agy update`, `copilot update` or `cursor-agent update`. Claude Code is installed on its stable
channel, a release about a week old. Because it lives in your home, the same copy also works inside
any toolbox or distrobox.

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
