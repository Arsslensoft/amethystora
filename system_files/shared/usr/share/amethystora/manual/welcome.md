# Welcome to Amethystora

Amethystora is a desktop you set up once and then simply use. It is built on Fedora and GNOME, updates
itself in the background, and an update that goes wrong is one restart away from being undone.

### One image, not a pile of packages

The whole operating system is a single image, built, tested and signed before it reaches your machine,
and replaced as a whole when a new one comes out. Your files, settings and apps sit beside it, untouched:
nothing to clean up, no half-finished upgrade to recover from, no reinstall every couple of years.

### A desktop for the keyboard

Windows tile side by side on a strip that scrolls, six workspaces are always where you left them, a theme
switch repaints everything at once, and an AI agent that knows how this system works is there when you
turn it on.

![Search anything](media/super-space.webm) ![Hold Super](media/hold-super.webm)

### Your first day

The [greeting](login.md#the-first-login) says hello, then **Pick a look** opens once on top of this page.
It sets the theme, the style, how the dock grows, the pointer and whether windows tile or float, with a
preview of each. `amethystora-control welcome` opens it again, and Control has every one of them later.

- **Floating windows**, as on most desktops: choose **Classic** there, or run `ame desktop layout classic`
  at any time ([Classic windows](tiling.md#classic-windows)). An account that did not see Pick a look is
  offered them in a notification shortly after its first login.
- **A computer for a child:** [parental controls](logins.md#parental-controls) keep their account to the
  apps and the hours you choose.

### Three things you can undo

- **An update:** choose the previous system in the boot menu; System Updates offers to keep it
  ([Rolling back](rolling-back.md)).
- **A file:** **Backups** shows your home folder as the hourly snapshots and the daily backup kept it,
  and puts back what was lost ([Backups](backups.md)).
- **Settings:** **Reset settings** on Control's System page puts yours, the machine's or both back as
  the image ships them, and keeps your files ([Resetting settings](starting-over.md#resetting-settings)).

## How this manual works

Read it front to back the first time; each page ends with a link to the next. After that, search:
`Ctrl+K` or `/` jumps to the search box, and the results go straight to the section you need.

- **Start here**: a new machine ready, and how to install.
- **The desktop**: everything you touch every day, from getting around and the hotkeys to themes, the
  terminal, web apps and the agent.
- **Software**: installing anything, from apps to developer toolchains, without damaging the system.
- **The system**: updates, security and hardware.
- **Help**: where to look when something is wrong.

This page opens by itself at your first login, and only then. `Super+F1` opens the manual from anywhere,
and so does "Manual" in the Amethystora menu (`Super+Alt+Space`). In a terminal, `amethystora-manual`
opens it and `amethystora-manual keybindings` goes straight to a page.

## What is yours and what is the image's

One idea explains most of how Amethystora behaves:

| Where | Whose | What happens to it |
| --- | --- | --- |
| `/usr` | The image | Read-only. Replaced whole by every update. |
| `/etc` | Both | The image's defaults, merged with your changes on every update. |
| `/var` and `/home` | Yours | Never touched by an update. |
| `~/.config`, `~/.local` | Yours | Your settings, per account. |

Apps come from Flathub, command line tools from Homebrew, and development environments live in
containers. None of them changes the image, so an update can never break them and they can never break
an update. [Installing software](software.md) explains each one.

> **In short:** keep the operating system pristine, keep your work beside it, and restart when you
> are done for the day. That is all the maintenance there is.
