# Plugins

A plugin adds something of its own to the desktop: a build's status in the pill, a package on its way, a door that is open, a timer, a
number on a dock icon. It runs in a *sandbox*, as a program of its own, and only describes what to show; the desktop draws it. It cannot do
more than you let it, and nothing it was approved with can change without an administrator's approval. The widgets of earlier images are
plugins now: see [widgets](widgets.md).

## Where a plugin shows

One plugin can ask for several places, and an agent learns one format to reach all of them.

![A plugin in the pill](media/plugin-pill.webm)

### In the live pill

A plugin comes after everything the system shows, so a call, a recording, a link from your phone or the battery's question is never hidden
by one. The one that changed last is in the pill, the others are rows in its card, and the card ends with the plugin's name, what it may
reach and a **Pause** button, drawn by the desktop whatever the plugin said. A countdown is counted by the desktop, so it costs no process,
and words marked **private** hide while your screen is shared.

### Everywhere else

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
  as any run. It is an ordinary window, drawn without the graphics card, that can see nothing of yours but what it was given, except what
  you copied last while the window has the focus: it can read that, and can look like any other window. So a plugin with a window never
  has the network, and the system refuses a plugin that asks for both. What the window reads can still reach any other plugin that shares
  a folder with it.
- **The AI agent's tools**: while the agent may make changes, it can run a tool a plugin offers, asking you each time, and reads what the
  plugin printed as data.

### What a plugin cannot look like

Nothing a plugin shows can look like the system: no call, recording, battery or agent look, no markup, no picture from a file and no field
to type in. A plugin whose output the desktop cannot draw is paused, not drawn.

A plugin's **settings** are in its row in [Control](control.md): each one you set is typed unseen, and a secret goes into the Vault. A theme
or look shared as a file (`.amethystora-theme`, `.amethystora-look`) is a plugin of data alone: `ame plugin add` hands it to
[the theme's own checks](themes.md), and nothing of it runs.

## A script, live in the pill

Any script, cron job or Makefile can be live in the pill with no folder and no setup:

```bash
ame pill start "Backup" --progress 0.1
ame pill update "Backup" --progress 0.4 --text "Copying photos"
ame pill end "Backup"
```

`--until` takes a time (seconds since 1970, or `+SECONDS` from now) which the desktop counts down, `--icon` a theme icon's name, and
`--private` hides it while your screen is shared. The pill goes when the script that started it ends, even if it forgets to end it.

It is your own script: nothing is sandboxed and nothing is asked. But what it says is drawn only as text, a few at once, and a script that
changes its pill more than four times a second for a minute has its pill ended.

## Made by the agent

With [the agent](ai-agent.md) on, ask it in words:

```bash
ame plugin make "show my GitHub build in the pill"
```

The agent writes a **draft**: it passes the same check as every plugin and shows at once, running with *nothing allowed* (no network, no
files), marked **Draft**, with the agent's sample of what it will print. Its card has **Keep**, **Change** and **Throw away**.

Nothing stays until you say so, and the agent can never give a draft anything: **Keep** asks you in a terminal what each permission is for,
as for any plugin, and an administrator approves.

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

Nothing of the plugin runs while you add it. Its files are first kept where only the system can change them, and you read the plugin's
name and author, a *digest* of its files, and what it asks for, each with its author's reason. You answer each permission on its own (the
plugin has to cope with a no), in a terminal, with no way to answer in advance. Then an administrator approves in the system's own dialog,
which names the plugin and each permission it is about to give.

### Checking a digest

If a page publishes a plugin's digest, add it with `--digest` and nothing else is accepted:

```bash
ame plugin add https://example.org/ci-build.git --digest 24d7ee22fb319619df0095cbf547bb884532306cb2294241771ddd9b37099c44
```

### Updates

A git address is taken at the version it has now, and `ame plugin update` shows what changed, and what is newly asked for, before
anything is installed. Nothing updates by itself. `ame plugin update --all` fetches every plugin you added from an address again and
installs, in one approval that names each of them, the new versions that ask for nothing more; one that asks for more is left for
`ame plugin update ID ADDRESS`, on its own, where you read each new permission.

### While you make one

If you are making a plugin that needs permissions, `ame plugin dev FOLDER` installs your folder, then installs it again each time you
save a change, with no more than you allowed it. That approval is kept five minutes, the only one that is: anything that asks for more
stops the loop.
