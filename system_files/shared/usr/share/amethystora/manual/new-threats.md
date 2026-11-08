# Looking for new threats

New malware has no virus signature yet, so these look for what it does rather than what it is. Each is off
until you turn it on, in the Settings page or with `ame security settings`. Most can be `record`, which
keeps what it finds in the history and the Events page without a notification, or `on`; the decoy key and
app integrity are only `on`, the hunting rules only `record`. None sends anything anywhere unless its own
entry says so.

What one of them finds about your own account, such as your apps, is said to you. The rest is said to the
administrators, who see all of it on the **Events** page.

## Odd processes

`processes`: every 15 minutes, what runs. Each is said the first time for that program:

- A program whose file was deleted while it runs, or that runs from memory alone, which malware does to
  leave nothing to scan.
- One in a temporary folder, `~/.cache` or your download folder.
- A process that names itself like a kernel thread but runs a program, which kernel threads never do.
- A network tool or an AI agent started by an npm package's install script, and an AI agent run with its
  own checks turned off (such as `--dangerously-skip-permissions` or `--yolo`) not started at a terminal.
- A library from outside the system loaded into a program with `LD_PRELOAD`, and a session whose own
  environment, which no file shows, has every program it starts load code from outside the system: both
  are how malware gets inside the app you type a password into without changing its files.

Flatpak's and Homebrew's updates, containers, AppImages and Go's tests are allowed already.

## Ransomware behaviour

`snapshot-watch`: ransomware and wipers, whatever their family, rewrite or delete many files at once.
After each hourly snapshot ([Ransomware protection](viruses.md#ransomware-protection), which this needs),
the watcher lists what changed since the one before, from the snapshots' bookkeeping alone.

When more than 300 files of one account changed, it looks at up to a hundred of them: a file that went
from readable to random is what encryption looks like, while photos and archives, already random, do not
count. Caches, build output and dependency folders are left out. Encryption, or more than 300 files
deleted, is critical, and its event has **Open in Backups**, at the snapshot from before it.

## Programs run from download folders

`exec-watch`: the audit log records each program run from a temporary folder, `~/.cache` or your download
folder, and each is said the first time: a download or a dropped file starting. Builds run thousands of
programs from `/tmp`, so `record` keeps it quiet while it learns. It takes effect from the next restart,
and a script started through its interpreter (`sh /tmp/x`) is seen by odd processes instead.

## AI agents run with their checks off

`agent-clis`: an AI agent started with a flag that has it do whatever it is told without asking, such as
`--dangerously-skip-permissions` or `--yolo`. The Nx attack of August 2025 ran agents that way from an npm
install script, to search the machine for secrets.

The audit log records each run of an agent from the folders it is installed in: one started with no
terminal is said, one you started at a terminal is recorded. It takes effect from the next restart, and
an agent installed since then is seen by odd processes instead. A script of yours that runs an agent on
purpose is allowed with **Allow…** on its event.

## Credential tripwires

`credentials`: password stealers, and the poisoned npm and PyPI packages that carried one, read keys and
tokens, send them out and exit, leaving nothing to scan. The audit log records each read of where secrets
are kept: SSH's keys, npm's, PyPI's, git's, GitHub's, the clouds' and the container registries' tokens, the
keyring, the browsers' passwords and cookies, and the AI agents' tokens.

A program reading one that is not its own is said, and one process reading several kinds is critical:
that is the sweep a stealer makes. SSH, git, the browsers, the agents, the backup, the virus scans and the
file indexer are allowed already. It records which programs read secrets, never what they read, and takes
effect from the next restart. Chromium's saved passwords are in a file whose name has a space, which the
audit log cannot watch.

## Programs reaching into others' memory

`injection`: a program that attaches to another, or reads or writes its memory, which is how malware gets
inside the app you type a password into, and every such try the kernel refused. A program cannot do it to
one it did not start, but it can to its own children, as a debugger must.

A debugger (`gdb`, `strace`, `perf`) and a program reaching into its own processes, such as a crash
reporter or two programs of one Flatpak, are recorded and not said. The file-watch sensor records each
where the kernel checks it, or the audit log on a kernel that will not run the sensor, from the next
restart.

## Decoy key

`decoy`: puts a fake SSH key, `~/.ssh/id_backup`, in each home folder. SSH never offers a key by that name
on its own, so nothing legitimate reads it, and any read of it is critical. It is a switch of its own
because it puts a file in your home folder; turned off, it is taken away again at the next restart. A key
of your own by that name is never touched, and then no decoy is made.

## Apps that gain permissions

`app-permissions`: a Flatpak app that gains a way out of its sandbox
([App permissions](app-permissions.md)), through an update, an override or another app in its place. An
app installed with its permissions is recorded; a permission added to one you have is said. It is read as
each account, for the accounts with a session in the last day, and said to that account alone.

## SELinux denials

`selinux-denials`: SELinux keeps each confined service to what its policy allows, and records what it
refused. A service suddenly refused something it never tried before is often what an exploit looks like
from outside. Each different denial is said the first time; one SELinux let through, because that
service's domain is permissive, is recorded. It needs the audit log on.

## AI agents' settings

`agent-config`: what your AI agents' own settings have them run by themselves. A hook runs a command at
every step the agent takes, and an MCP server is a program it starts, or an address it sends to: planted
in its settings, either runs the next time you ask it anything.

Claude Code's, Codex's, Antigravity CLI's, Gemini CLI's, Opencode's, Cursor's and Copilot's are read, with
the commands their settings run (a status line, a key helper), the plugins and extensions they load, and
a permission mode that turns their checks off. Folders and tools you trust are recorded. Read as each
account, like app permissions. Only your own settings are read: read a repository's agent files
(`.claude/`, `.mcp.json`, `.vscode/tasks.json`) before you open it with an agent.

## Programs in the kernel

`bpf-programs`: eBPF rootkits load small programs into the kernel that hide files, processes and
connections. systemd and the firewall load the same few every time, so a new one is recorded, and said
when a program outside the system loaded it, or when it watches what the kernel does and nothing is left
holding it. A rootkit that also hides its programs from this is not caught; most do not.

## Startup hooks in developer tools

`dev-hooks`: code put where a developer tool runs it by itself. Read as each account, like app
permissions; virtual environments are not looked into.

- A Python `.pth` file whose lines start with `import` runs them at every start of Python, which is how a
  poisoned package stays after it is removed.
- git's `core.hooksPath`, `core.fsmonitor` and `core.sshCommand` run in every repository.
- SSH's `ProxyCommand`, `LocalCommand` and `Match exec` run on connecting.
- npm's `script-shell` and `node-options` change what npm and Node run.
- A new VSCodium extension is said too, and its updates, which it installs by itself, are recorded.

## Behaviour sensor

`behaviour`: Falco, which sees what every process does as the kernel sees it (each command, file opened
and connection) through an eBPF probe, and matches it against its rules: a shell started by a server, a
secret read by a program that has no business with it, a binary dropped and run. The watcher reads what
they notice.

Unlike the others it runs all the time, for a little CPU and a few hundred megabytes of memory, and its
rules are the ones its package ships, never downloaded. If the probe cannot load into the running kernel,
the report says so under **Detection services**.

## Chains of steps

`chains`: an attack is steps that each look harmless alone: a program run from the downloads, a token
read, a connection out. Each finding names its account and its step, and this puts them together: two
different steps for one account within half an hour are said, and three, or reading secrets and then
reaching a command server, are critical. What the others only record counts too, so they can stay quiet
and still help; what you allowed does not. Each chain is said once, and again only when it grows.

## Hash reputation

`ame security settings reputation`, off until you choose it: each program that odd processes or the
download folders' watch saw run for the first time is looked up with MalwareBazaar by its hash, and it
says at once if it is known malware.

It sends only the hash, never the file, its name or its path, to abuse.ch, with your own free key from
[auth.abuse.ch](https://auth.abuse.ch/), which the command asks for and keeps where only root can read it.
A hash can still say which files you have, and the lookups made with one key say when new programs ran,
which is why you choose it on its own. It knows only malware someone has already sent in, and changing one
byte of a program defeats it: it catches nothing new, only the old sooner.

## Asking the agent about events

`ame security settings triage`, off until you choose it: each event on the Events page has **Ask the
agent**, and your AI agent explains, in a terminal, what it most likely is and how sure it is, reading
only and changing nothing. The event, and what the agent then reads on this machine, goes to the agent's
maker, which is why you choose it on its own. An event is written partly by whatever it is about, so the
agent is told to read it as data, never as instructions.
