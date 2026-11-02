# Welcome to Amethystora

Amethystora is a desktop you set up once and then simply use. It is built on Fedora and GNOME, it
updates itself in the background, and an update that goes wrong is one restart away from being
undone.

It is not a pile of packages you have to keep in order. The whole operating system is a single
image, built, tested and signed before it ever reaches your machine, and replaced as a whole when a
new one comes out. Your files, your settings and your apps sit beside it, untouched. That is why
there is nothing to clean up, no half-finished upgrade to recover from, and no reinstall every
couple of years.

On top of that foundation sits a desktop made to be driven from the keyboard, as beautiful as it is
fast: windows that tile side by side on a strip that scrolls, six workspaces that are always where you left them, a theme
switch that repaints everything at once, and an AI agent that knows how this system works, for when you turn it on.

Two choices are worth making on the first day, once the [greeting](desktop.md#the-first-login) has said hello. **Pick a look**, which opens once on top of this page at your
first login, sets the theme, the style, how the dock grows, the pointer and whether windows tile or float, with a
preview of each as you pick; `amethystora-control welcome` opens it again, and Control has every one of them
later. If you would rather have windows that float and overlap, as on Windows and macOS, choose **Classic** there,
or `ame desktop layout classic` at any time ([Classic windows](desktop.md#classic-windows)); an account that did
not see Pick a look is offered them in a notification shortly after its first login.
If the computer is for a child, [parental controls](security.md#parental-controls) keep their account
to the apps and the hours you choose.

Three things can be undone, each where you would look for it. An update: choose the previous system
in the boot menu, and System Updates offers to keep it ([Rolling back](updates.md#rolling-back)). A
file: **Backups** shows your home folder as the hourly snapshots and the daily backup kept it, and puts
back what was lost ([Backups](security.md#backups)). Settings: **Reset settings** on Control's System
page puts yours, the machine's or both back as the image ships them, and keeps your files
([Resetting settings](troubleshooting.md#resetting-settings)).

## How this manual works

Read it front to back the first time; each page ends with a link to the next. After that, search:
`Ctrl+K` or `/` jumps to the search box, and the results go straight to the section you need.

- **Start here** gets a new machine ready: the first things to do, and how to install.
- **The desktop** is everything you touch every day: getting around, the hotkeys, themes, the
  terminal, web apps and the agent.
- **Software** is how to install anything, from apps to developer toolchains, without damaging
  the system.
- **The system** covers updates, security and hardware.
- **Help** is where to look when something is wrong.

This page opens by itself the first time you log in, and only then. `Super+F1` opens this manual from
anywhere, and so does "Manual" in the Amethystora menu (`Super+Alt+Space`). From a terminal,
`amethystora-manual` opens it and `amethystora-manual keybindings` goes straight to a page.

## What is yours and what is the image's

One idea explains most of how Amethystora behaves:

| Where | Whose | What happens to it |
| --- | --- | --- |
| `/usr` | The image | Read-only. Replaced whole by every update. |
| `/etc` | Both | The image's defaults, merged with your changes on every update. |
| `/var` and `/home` | Yours | Never touched by an update. |
| `~/.config`, `~/.local` | Yours | Your settings, per account. |

Apps come from Flathub, command line tools from Homebrew, and development environments live in
containers. None of them change the image, which is why an update can never break them and they can
never break an update. [Installing software](software.md) explains each one.

> **In short:** keep the operating system pristine, keep your work beside it, and restart when you
> are done for the day. That is all the maintenance there is.
