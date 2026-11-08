# Control

Most of what you can change about Amethystora is a command in the terminal, and a command nobody tells you about is one
you never use. **Control** puts those settings in one window.

## Opening Control

`Super+Ctrl+Shift+C`, **Control** in the app grid, or **Control** in the Amethystora menu. From a terminal:

```bash
amethystora-control desktop          # a page: appearance, desktop, keyboard, apps, agent, system or setup
amethystora-control desktop focus    # one setting's card on a page
amethystora-control welcome          # Pick a look
```

**Pick a look** is the page a new account sees once: the theme, the style, the dock, the pointer and the windows, with
a preview of the desktop as you pick.

## How a change works

Each setting says what it is now, what it changes, and the page of this manual it is on. A change runs the same `ame`
command you could type yourself, shown under the card, so nothing in Control is out of reach of a terminal.

A change that asks something, explains a risk first or needs your password opens in a terminal, marked **Terminal** on
its card; Control reads everything again when you come back to it.

## What is where

| Page | Settings |
| --- | --- |
| **Appearance** | The [theme](themes.md#switching), with a preview of each one you point at, its [accent](themes.md#accents), [colours from the wallpaper](themes.md#colours-from-your-wallpaper), its [wallpapers](wallpapers.md) and [Daylight](wallpapers.md#daylight), [an accent per workspace](themes.md#an-accent-per-workspace), the [transition](wallpapers.md#transitions) that reveals a new one and how long it takes, the [reveal at login](login.md#the-reveal-at-login), the [style](styles.md#styles), the [font](styles.md#fonts), the [icons](styles.md#icons), [rounded screen corners](styles.md#rounded-screen-corners), the [boot menu](make-a-theme.md#the-boot-menu)'s artwork, and the desktop's [sounds](sound.md#sounds) |
| **Desktop** | The [window layout](tiling.md), the [recently used apps](search.md#the-app-grid) in the app grid, the one-time [tips](desktop.md#tips), how long the [clipboard history](clipboard.md#the-clipboard) is kept, when the [trash](files.md#the-trash) empties itself, the [widgets](widgets.md) in the top bar, the terminal's [greeting](terminal.md#the-greeting) and its [tools](terminal.md#command-line-tools), [phone pairing](wireless.md#phones), whether the clipboard is shared with it and where its [photos](phone-sharing.md#photos) go, [ringing it](phone-calls.md#ringing-the-phone), [music from either side](phone-calls.md#music-from-either-side) and [the phone as a slide clicker](phone-calls.md#the-phone-as-a-slide-clicker), and a link to the [hotkeys](../keybindings.md) |
| **Keyboard** | [Every shortcut](keyboard.md#every-shortcut), GNOME's, the desktop's, the tiling's and your own, changed by pressing the new keys; the [keyboard settings](keyboard.md#keyboard-settings) GNOME keeps out of sight: Caps Lock, the Compose key, what Super alone opens, middle-click paste, Num Lock and the repeat delay; [a layout for each window](keyboard.md#a-layout-for-each-window); and [holding a key for accents](keyboard.md#hold-a-key-for-accents) |
| **Apps** | [Games](games.md#games), [Android apps](games.md#android-apps), [drawing tablets](devices.md#drawing-tablets), [the AUR](containers.md#the-aur), [web apps](web-apps.md), other game stores once Steam is in, starting and stopping Android once it is set up, and add-ons that install with one command: JetBrains Toolbox, the command-line tools, Homebrew's collections, the CNCF's tools and the apps the image comes with |
| **AI agent** | Whether the [AI agent](ai-agent.md) is on (it is off until you turn it on), which one, and installing it; whether it may [make changes](ai-agent.md#what-it-can-do), and [what it changed](agent-control.md#history-and-undo); and, for every account and with an administrator's password, how it runs [root commands](agent-control.md#root-commands) and [the lock](agent-control.md#the-lock) on how far anyone may turn it on |
| **System** | The [image and the stream](streams.md#switching-streams-and-images) this machine follows, [developer mode](developer.md#turning-it-on) and, while it is on, its [groups](developer.md#groups), [an image of your own](own-image.md), the [CPU scheduler](games.md#a-faster-cpu-scheduler-for-games), how [swapped-out memory](hardware.md#memory) is compressed, restarting into [the firmware's settings](hardware.md#secure-boot), [freeing disk space](common-problems.md#the-disk-is-filling-up), the [pools of disks](disks.md#pools-of-disks-raid) and [the system's own disks](disks.md#the-systems-own-disks) in a **Disks** group, where each says how it is doing and offers what it needs, [resetting settings](starting-over.md#resetting-settings) and [starting over](starting-over.md#starting-over) |
| **Your setup** | Your [setup file](setup.md): saving it, what this machine and the file disagree on, and applying it; and, once a [backup](backups.md) is set up, making the setup again from one |

The search box finds a setting on any page by what it is called or what it does.

## What is not here

- **Security settings** are in Amethystora Security, whose report says what each protection does and
  turns it on ([Security](security.md#see-where-you-stand)). Control's sidebar opens it. The one exception
  is the AI agent's root commands and lock, which govern the agent rather than protect the machine.
- **Automatic updates** are switched in Updates, which shows when the next one comes
  ([Updates](updates.md)). Control's sidebar says whether they are on, and opens it.
- **Backups**, setting one up, backing up now and the daily run, are in Backups, which also puts files
  back ([Backups](backups.md)). Control's sidebar opens it.
- **GNOME's own settings**, such as displays, sound, networks and accounts, are in GNOME's Settings.

## In a terminal

`ame control` lists every setting, what it is now, and the command that changes it. Those commands take their choice
as an argument, so a script can use them too:

```bash
ame desktop layout classic
ame desktop greeting off
ame agent use opencode
ame apps aur off
```
