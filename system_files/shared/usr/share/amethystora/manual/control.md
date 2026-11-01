# Control

Most of what you can change about Amethystora is a command in the terminal, and a command nobody tells
you about is one you never use. **Control** puts those settings in one window: `Super+Ctrl+Shift+C`,
**Control** in the app grid, or **Control** in the Amethystora menu. `amethystora-control PAGE` opens it at
a page: `appearance`, `desktop`, `apps`, `agent`, `system` or `setup`, and `amethystora-control PAGE ENTRY` at one
setting's card on it (`amethystora-control desktop focus`). `amethystora-control welcome` opens
**Pick a look**, the page a new account sees once: the theme, the style, the dock, the pointer and the windows, with
a preview of the desktop as you pick.

Each setting says what it is now and what it changes, and the page of this manual it is on. A change
runs the same `ame` command you could type yourself, which the card shows under it, so nothing in
Control is out of reach of a terminal, and `ame control` lists every setting there. A change that asks
something, explains a risk first or needs your password opens in a terminal, marked **Terminal** on its
card, and Control reads everything again when you come back to it.

## What is where

| Page | Settings |
| --- | --- |
| **Appearance** | The [theme](themes.md#switching), with a preview of each one you point at, its [accent](themes.md#accents), [colours from the wallpaper](themes.md#colours-from-your-wallpaper), its [wallpapers](themes.md#wallpapers) and [Daylight](themes.md#daylight), [an accent per workspace](themes.md#an-accent-per-workspace), the [transition](themes.md#transitions) that reveals a new one and how long it takes, the [reveal at login](desktop.md#the-reveal-at-login), the [style](themes.md#styles), the [font](themes.md#fonts), the [icons](themes.md#icons), [rounded screen corners](themes.md#rounded-screen-corners), the [boot menu](themes.md#the-boot-menu)'s artwork, and the desktop's [sounds](desktop.md#sounds) |
| **Desktop** | The [window layout](desktop.md#tiling-windows), the [recently used apps](desktop.md#the-app-grid) in the app grid, the one-time [tips](desktop.md#tips), how long the [clipboard history](desktop.md#the-clipboard) is kept, when the [trash](desktop.md#the-trash) empties itself, the [widgets](widgets.md) in the top bar, the terminal's [greeting](terminal.md#the-greeting) and its [tools](terminal.md#command-line-tools), [phone pairing](hardware.md#phones), whether the clipboard is shared with it and where its [photos](phone.md#photos) go, [ringing it](phone.md#ringing-the-phone), [music from either side](phone.md#music-from-either-side) and [the phone as a slide clicker](phone.md#the-phone-as-a-slide-clicker), and a link to the [hotkeys](../keybindings.md) |
| **Apps** | [Games](games.md#games), [Android apps](games.md#android-apps), [drawing tablets](hardware.md#drawing-tablets), [the AUR](software.md#the-aur), [web apps](web-apps.md), other game stores once Steam is in, starting and stopping Android once it is set up, and add-ons that install with one command: JetBrains Toolbox, the command-line tools, Homebrew's collections, the CNCF's tools and the apps the image comes with |
| **AI agent** | Whether the [AI agent](ai-agent.md) is on (it is off until you turn it on), which one, and installing it; whether it may [make changes](ai-agent.md#what-it-can-do), and [what it changed](ai-agent.md#history-and-undo); and, for every account and with an administrator's password, how it runs [root commands](ai-agent.md#root-commands) and [the lock](ai-agent.md#the-lock) on how far anyone may turn it on |
| **System** | The [image and the stream](updates.md#switching-streams-and-images) this machine follows, [developer mode](developer.md#turning-it-on) and, while it is on, its [groups](developer.md#groups), [an image of your own](developer.md#an-image-of-your-own), the [CPU scheduler](games.md#a-faster-cpu-scheduler-for-games), how [swapped-out memory](hardware.md#memory) is compressed, restarting into [the firmware's settings](hardware.md#secure-boot), [freeing disk space](troubleshooting.md#the-disk-is-filling-up), the [pools of disks](hardware.md#pools-of-disks-raid) and [the system's own disks](hardware.md#the-systems-own-disks) in a **Disks** group, where each says how it is doing and offers what it needs, [resetting settings](troubleshooting.md#resetting-settings) and [starting over](troubleshooting.md#starting-over) |
| **Your setup** | Your [setup file](setup.md): saving it, what this machine and the file disagree on, and applying it; and, once a [backup](security.md#backups) is set up, making the setup again from one |

The search box finds a setting on any page by what it is called or what it does.

## What is not here

- **Security settings** are in Amethystora Security, whose report says what each protection does and
  turns it on ([Security](security.md#see-where-you-stand)). Control's sidebar opens it. The one exception
  is the AI agent's root commands and lock, which govern the agent rather than protect the machine.
- **Automatic updates** are switched in Updates, which shows when the next one comes
  ([Updates](updates.md)). Control's sidebar says whether they are on, and opens it.
- **Backups**, setting one up, backing up now and the daily run, are in Backups, which also puts files
  back ([Backups](security.md#backups)). Control's sidebar opens it.
- **GNOME's own settings**, such as displays, sound, networks and accounts, are in GNOME's Settings.

## In a terminal

```bash
ame control           # every setting, what it is now, and the command that changes it
```

The commands it names take their choice as an argument, so a script can use them too:

```bash
ame desktop layout classic
ame desktop greeting off
ame agent use opencode
ame apps aur off
```
