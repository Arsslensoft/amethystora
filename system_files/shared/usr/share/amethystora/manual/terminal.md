# The terminal

Amethystora is made for people who like the command line as much as for people who never open it. When you
do, it is ready for you.

## Opening one

`Super+Return` opens a terminal, and so do `Ctrl+Alt+T` and `Ctrl+Alt+Return`, the shortcuts most Linux
desktops use ([Hotkeys](../keybindings.md#launching)), **Terminal** in the app grid, and **Open in Console**
on a folder in Files.

### kitty, and Ptyxis

The terminal is kitty, a fast terminal that draws on the graphics card. Its settings are the image's, in
`/etc/xdg/kitty/kitty.conf`: put a line of your own in `~/.config/kitty/kitty.conf` and yours wins, the
rest stays as the image has it.

Ptyxis is still installed, without a launcher of its own: run `ptyxis` to open it. It is the one with a
menu that opens a tab inside any of your [Distrobox and Toolbox containers](containers.md), and the second
choice when kitty is missing.

### Colours and the prompt

Its colours follow the [theme](themes.md), open windows included when you switch, and so does the prompt:
your name and machine, the folder, and in a git repository its branch, each on a background of the theme's
colours, then the command on a line of its own. The arrows and icons are Nerd Fonts symbols, which kitty
carries and Ptyxis finds among the image's fonts.

### Splits, search and picking

A new window, tab or split opens in the folder you are in. `Ctrl+Shift+Return` splits the window in front
along its longer side, `Ctrl+Shift+]` and `Ctrl+Shift+[` move between the splits, and `Ctrl+Shift+L` shows one
at a time and back. A tab whose window printed something while it was behind carries a ●. Pointing at a link
shows where it really goes.

`Ctrl+Shift+/` searches what scrolled past, `Ctrl+Shift+G` shows the last command's output alone,
`Ctrl+Shift+E` picks a link to open with the keyboard, `Ctrl+Shift+P` then `F` a path to type, and
`Ctrl+Shift+U` a character by its name. `Ctrl+Shift+F3` lists every action and its keys.

## The greeting

The first terminal you open in a session says hello with the Amethystora logo, a summary of the machine,
the image you are running, a few useful commands and a tip; the ones after it just say welcome, so they
open sooner. `ame desktop greeting off` turns it off, `on` brings it back, and without either it switches;
so does the switch on [Control](control.md)'s **Desktop** page.

### fetch

`fetch` shows the logo and the system summary again whenever you like; anything you add after it goes to
fastfetch, which it runs. A glint crosses the logo's facets before the summary appears. You get the still
logo instead over SSH, in the text console, in a terminal without full colour, with `NO_COLOR` set, or with
**Reduce Animation** on in **Settings → Accessibility**. `fastfetch` itself, and `neofetch`, show the same
summary beside a logo picked at random, as `/etc/amethystora/fastfetch.json` says.

### The lock in the summary

The image's name ends with a lock. Closed, 🔐, this machine takes only updates signed with Amethystora's
key; open and blinking, 🔓, it takes its updates unchecked, and `ame system rebase` switches it back
([Signed updates](streams.md#signed-updates)).

## Shells

The shell is bash, with the Starship prompt; fish and zsh are installed too, with the same prompt. You
switch in the terminal rather than system-wide, so that a broken shell configuration can never lock you out
of logging in: put `shell /usr/bin/fish` or `shell /usr/bin/zsh` in `~/.config/kitty/kitty.conf`, and open a
new window.

## Told when a command ends

Start something long, a build or a download, and go and read something else. When a command that ran ten
seconds or more ends while its terminal is behind, a notification says so, and clicking it brings the
terminal to the front.

![A long command ends, and a notice says so](media/command-done.webm)

It is kitty's own setting (`notify_on_cmd_finish`), and the words are kitty's: the command, never what it
printed. `ame desktop command-done off` turns it off and `on` back on; Control has it as **Told when a long
command ends**. A line of your own in `~/.config/kitty/kitty.conf` wins over both.

## A missing command

Type a command this computer does not have, and the terminal can say where it can come from instead of a
bare *command not found*:

```text
htop is not on this computer. It can come from:
  A container  ame pkg containers new --template fedora --name fedora && ame pkg fedora install htop
Nothing was installed.
```

It is off until you turn it on with `ame desktop missing-command on`, or **Where a missing command comes
from** in Control; new terminals follow it, in bash, zsh and fish.

### Where it looks

Only at what is already on this computer: Fedora's packages as the system was built (for a
[container](software.md)), Homebrew's list of formulas once Homebrew has fetched it, the apps Flathub lists
once Flatpak has fetched its list, and the programs an `ame` command brings, such as Steam. It never
installs anything and asks nothing of the network, so it says *can come from*: a list can be out of date.

## ame

`ame` runs the commands that come with the system: small, tested scripts for the jobs that would otherwise
take a web search, in groups such as `desktop`, `security` and `system`. This manual mentions them where
they help; to see all of them:

```bash
ame                           # the groups, and the commands outside them
ame security                  # the commands in one group, with a line about what each does
ame --list --list-submodules  # every command, in every group
ame -n security status        # print what a command would run, without running it
```

`ame pkg` is [amethystora-pkg](containers.md#packages-from-other-distributions). `ujust`, the older name of
`ame`, runs the same commands, and their names from before they were grouped, such as `ujust setup-backup`,
still work.

The commands are `just` recipes, and `just` itself is yours for your own projects: a `justfile` in any folder
turns its commands into recipes the same way.

## Command line tools

Command line tools come from Homebrew, which installs into your home without `sudo` and without touching the
system, and is updated along with it. Never run it with `sudo`: it is yours, not root's. Browse what there
is at [formulae.brew.sh](https://formulae.brew.sh).

```bash
brew install ripgrep
brew search <name>
brew upgrade
```

### Already on the image

`just`, `gum`, `glow`, `tmux`, `fastfetch`, `git` and the rest of the usual tools, and `open`, which opens a
file or a link in its app, as on a Mac. With RamaLama installed from Homebrew, `rl` runs it. The fonts
include Inter, JetBrains Mono and the Nerd Fonts symbols, so prompts and file managers in the terminal draw
their icons.

### Newer tools in place of the usual ones

`ame apps cli` installs a set of newer tools from Homebrew, eza, bat, ripgrep, zoxide and more, and sets
your shell up to use them: `ls` lists with eza, `cat` shows with bat, `grep` searches with ugrep. It also
installs direnv, mise, atuin, gh and more, and the Rust versions of the core tools (`cp`, `mv`, `find`,
`diff` and the rest), which then come first in your account's `PATH`; uninstalling `uutils-coreutils` puts
the system's own back.

`ame desktop bling off` takes that out of your shell's settings and `on` puts it back; the **Terminal
tools** switch on Control's **Desktop** page does the same.
