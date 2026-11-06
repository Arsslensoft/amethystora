# Plugins

A plugin adds something of its own to the desktop: a build's status in the pill, a package on its way, a door that is open, a timer, a
number on a dock icon. It runs in a *sandbox*, as a program of its own, and only describes what to show; the desktop draws it. It cannot do
more than you let it, and nothing it was approved with can change without an administrator's approval. The widgets of earlier images are
plugins now: see [widgets](widgets.md).

## Where a plugin shows

One plugin can ask for several places, and an agent learns one format to reach all of them:

- **The live pill.** After everything the system shows, so that a call, a recording, a link from your phone or the battery's question is never
  hidden by a plugin. The one that changed last is in the pill, the others are rows in its card, and the card ends with the plugin's name, what
  it may reach and a **Pause** button, drawn by the desktop whatever the plugin said. A countdown is counted by the desktop, so it costs no
  process, and a plugin can mark its words **private**: they hide while your screen is shared.
- **The top bar**, as a label, an icon and a menu.
- **Quick settings**, as a tile with a switch and a value.
- **Search** (`Super+Space`): an answer when you type the plugin's prefix. What you type never reaches the plugin.
- **The dock**: a number on a pinned app's icon, and a bar along its bottom.
- **The desktop**: a small card on the wallpaper, while *Plugins on the desktop* is on in [Control](control.md).
- **Today**, the calendar menu: a card under your events.
- **The logo menu**: a line at its end, which runs the plugin's own action when you click it.
- **Files**: *Give to a plugin* in the Scripts menu of a file, while a plugin that takes files is on. Your computer asks you first, naming
  the plugin and the file, and the plugin gets that file's contents and nothing else.
- **Notifications**, under the plugin's name, at most one a minute, with one button of the plugin's own.
- **A shortcut** that runs one of its actions, with Super, or Control and Alt both. A shortcut that anything already uses, yours included,
  is never taken: the plugin goes without, and you are told once.
- **A window of its own**, which opens when you click its action and stays until you close it (eight hours at most), with the same limits
  as any run. It is an ordinary window, drawn without the graphics card, that can see nothing of yours but what it was given.
- **The AI agent's tools**: while the agent may make changes, it can run a tool a plugin offers, asking you each time, and reads what the
  plugin printed as data.

A theme or a look shared as a file (`.amethystora-theme`, `.amethystora-look`) is a plugin of data alone: `ame plugin add` hands it to
[the theme's own checks](themes.md), and nothing of it runs.

A plugin's **settings** are in its row in [Control](control.md): each one you set is typed unseen, and a secret goes into the Vault.

Nothing in it can look like the system: a plugin has no call, recording, battery or agent look, no markup, no picture from a file and no
field to type in. A plugin whose output the desktop cannot draw is paused, not drawn.

## A script, live in the pill

Any script, cron job or Makefile can be live in the pill with no folder and no setup:

```bash
ame pill start "Backup" --progress 0.1
ame pill update "Backup" --progress 0.4 --text "Copying photos"
ame pill end "Backup"
```

`--until` takes a time (seconds since 1970, or `+SECONDS` from now) which the desktop counts down, `--icon` a theme icon's name, and
`--private` hides it while your screen is shared. The pill is tied to the script that started it, so it goes when the script ends, even if
the script forgets to end it. It is your own script: nothing about it is sandboxed and nothing is asked, but what it says is drawn only as
text, a few at once, and a script that changes its pill more than four times a second, for a minute, has its pill ended.

## Made by the agent

With [the agent](ai-agent.md) on, ask it in words:

```bash
ame plugin make "show my GitHub build in the pill"
```

The agent writes a **draft**. It passes the same check as every plugin and appears in the pill at once, running with *nothing allowed* (no
network, no files) and marked **Draft** wherever it shows, with the agent's own sample of what it will print. Its card has **Keep**,
**Change** and **Throw away**. Nothing stays until you say so, and the agent can never give a draft anything: **Keep** asks you, in a
terminal, what each permission is for, as for any plugin, and an administrator approves.

[Control](control.md), **Desktop**, lists *Made by the agent*: every plugin and draft it wrote, when, and where it shows, each with **Fix**,
**Pause** or **Remove**, and **Remove all**. Nothing piles up behind your back.

## When one stops itself

A plugin is paused, and says so once, when it fails five runs in a row, takes the whole 30 seconds three times, goes over its memory twice or
updates too often. Nothing else is affected, and the pill's notice offers **Ask the agent to fix it** while the agent is on (`ame plugin fix ID`).
A plugin whose secret settings are in a closed [Vault](vault.md) waits and says so; it carries on once the Vault is open.

## Share one as a file

A plugin is a folder, and a folder can be sent as a file:

```bash
ame plugin share site-deploy        # writes site-deploy.amethystora-pill in your home folder
ame plugin open site-deploy.amethystora-pill
```

Double-clicking such a file opens a terminal that shows exactly what it runs and reaches, then asks. Nothing runs before you say yes, and
nobody's signature stands in for reading it.

## Settings and secrets

A plugin may ask you for settings, such as a repository or an access token. `ame plugin settings ID` says which, and `ame plugin set ID NAME`
asks for the value without showing it. Every one, a token or a tracking number, is kept in your [Vault](vault.md) and nowhere in the clear (an account with no Vault keeps only the plain ones in a file, and refuses a secret), and reaches the
plugin on a pipe, never in a command line or an environment variable.

## The examples

`ame plugin shipped` lists the plugins that come with Amethystora (a clock, a calendar, the system's load, a pomodoro timer in the pill,
Do Not Disturb, what plays, hello); adding one takes a click, you read what it asks for first, and none is on until you add it.
`ame plugin examples` lists more to start from, which reach a service (a GitHub build, a parcel, a game's score, a Home Assistant door, a
stock): `ame plugin new my-build ci-build` makes a draft of your own from one.

## Add one

```bash
ame plugin add https://example.org/ci-build.git
ame plugin add ./ci-build
ame plugin add ci-build.amethystora-plugin
```

Nothing of the plugin runs while you add it. The files are first kept where only the system can change them, and you read what
they are: the plugin's name and who wrote it, a *digest* of its files, and what it asks for, each with the reason its author gave.
You answer each permission on its own, and the plugin has to cope with a no. This question is asked in a terminal and has no way to
be answered in advance. When you have decided, your computer asks an administrator to approve, in its own dialog, which names the
plugin and each permission it is about to give.

If a page publishes the digest of a plugin, add it with `--digest` and nothing else is accepted:

```bash
ame plugin add https://example.org/ci-build.git --digest 24d7ee22fb319619df0095cbf547bb884532306cb2294241771ddd9b37099c44
```

A git address is taken at the version it has now, and `ame plugin update` shows what changed, and what is newly asked for, before
anything is installed. Nothing updates by itself.

## What a plugin may ask for

| Permission | What it gives |
|---|---|
| `network` | HTTPS to named hosts, and only those. No other address, no other port |
| `files` | One place under your home folder, read-only unless it says otherwise. Never the home itself, a hidden folder or the private folder |
| `notify`, `open`, `clipboard-write`, `pick` | A notice under its name, an address to open after a click, text for the clipboard after a click, a file you choose |
| `state`, `control` | What the system says of the clock, the disk, what plays, and the few controls the desktop offers |
| `events` | Runs when the network, the power, the screen's lock, the light or dark style, or (asked for on its own) the app in front changes |
| `network-local` | One service on your home network, by name and port, only while you are on a network you named home |
| `change` | Settings of Control's it names, after a click, each recorded with its way back as the agent's changes are |

| `window` | A window of its own, over Wayland only: no X11, no screen or sound capture, no portals |
| `key` | One shortcut, which never takes one already in use |

Never
grantable: root, a raw bus, the whole home, another plugin's data, the keyring, the screen or microphone, and input.

Two permissions together are said aloud: `files` with `network` means the plugin can send those files to those hosts. An allowed
host is a way out, up to its limit of 1 MiB an hour unless it asked for more and you agreed.

## See, check, take back

```bash
ame plugin list              # what is added, on or off
ame plugin show ID           # what it may do, what it asked for, what is blocked, and how it is sandboxed
ame plugin verify            # every file against what you approved
ame plugin history           # what was added, allowed, blocked and removed, and when
ame plugin revoke ID PERM    # less (all of it, with no PERM); no password needed
ame plugin block ID [HOST]   # stop what it sends, to a host or to all; at once, and what is open is cut
ame plugin unblock ID        # an administrator approves, so that nothing running as you can lift a block
ame plugin grant ID          # allow more of what it asked for: asks in a terminal, and an administrator approves
ame plugin rollback ID       # the version before its last update, with no more than you allowed
ame plugin on ID             # run it, within what it was given
ame plugin run ID [ACTION]   # one run now, as a script may: it opens, copies and changes nothing
ame plugin off ID
ame plugin remove ID         # gone, with what it kept
ame plugin check FOLDER      # what installing would refuse, for a plugin's author
```

If a file of a plugin changes after you approved it, the plugin stops and `ame plugin show` says so: only you decide what to do next.
Security's **Apps** page lists your plugins with what each was given, and blocks one's network or takes back what it was given in a
click, with no password. With [Plugins](security.md#plugins) on in Security, the security watcher says so too, and the report counts what it found.
`ame security plugin-approval off` lets an administrator install a plugin without the dialog, which also lets any program running as
that administrator do it: each plugin installed that way is recorded in `ame plugin history` and said in a notification.
Plugins are kept in `/var/lib/amethystora/plugins`, which only the system can write, and an administrator can turn them all off
or cut their network in `/etc/amethystora/plugins.conf`.

## What the sandbox does not promise

- It shares the kernel with everything else: a hole in the kernel is a way out.
- It protects your account from the plugin, and the store protects the plugin's code and grants from your account. Neither
  gets back what a program already running as you can read by itself.
- A host you allowed is a way out for whatever the plugin knows, up to its limit. Only a block stops that completely, because
  what goes through HTTPS cannot be read on its way.
- It says the plugin is what you approved, not that it is good.

See [security](security.md) for the rest of what protects this computer.
