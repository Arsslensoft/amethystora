# Containers

For anything that expects a traditional distribution (a compiler toolchain, a `.deb` a vendor ships, a
package only on the Arch User Repository, an Ubuntu-only tool), make a container. It shares your home
folder and your display, so its apps open windows like any other, and it can be thrown away without a
trace. Nothing installed in one touches the system.

![A container beside the system, its app in the grid](media/container-beside.webm)

> **A container is not a sandbox.** What you install in one runs as you, with your home folder, your
> display, your sound and your session bus. When an app is on Flathub, have it from there: Flatpak apps
> are sandboxed, signed and updated with the system.

## Packages from other distributions

`amepkg` (short for `amethystora-pkg`, and also `ame pkg`) makes containers from templates (`debian`,
`ubuntu`, `fedora`, `arch`, `alpine`, `opensuse-tumbleweed` and `opensuse-leap`) and installs into them with
their own package managers:

```bash
amepkg containers new --template debian
amepkg debian install gimp
amepkg debian enter
```

The first line downloads the image the template names, makes the container and brings it up to date.
`install` installs from Debian's own signed repositories with `apt`, and puts every app the package brought
in the app grid, marked "(on debian)"; `remove` takes them out again. Before it installs an app, `amepkg`
looks for the same app on Flathub and offers that first.

### Commands

| Command | Does |
| --- | --- |
| `amepkg containers list` | The containers, their templates, when each was last upgraded, their apps |
| `amepkg containers new` | A new container: it asks for the template and the name, or takes `--template`, `--name`, `--home <folder>` for a home of its own, and `--init` for one that runs services |
| `amepkg containers rm <name>` | Deletes the container and everything installed in it, after asking (`--force` outside a terminal); a home folder of its own is deleted only if you say so |
| `amepkg containers reset <name>` | Deletes it and makes it again from its template; its home folder stays, and it prints what you had installed with the command that installs it again |
| `amepkg <name> install`, `remove` or `purge <package>` | Installs or removes packages; `--yes` answers the package manager's questions, and `--no-export` keeps the apps out of the app grid |
| `amepkg <name> search <words>`, `show <package>` or `list` | Looks through its repositories, or what is installed |
| `amepkg <name> update`, `upgrade`, `autoremove` or `clean` | Refreshes its package lists, upgrades it, tidies up |
| `amepkg <name> enter` or `run <command>` | A shell in it, or one command |
| `amepkg <name> start` or `stop` | Starts or stops it |
| `amepkg <name> export --app <app>` or `--bin <path>` | Puts an app in the app grid, or a command in `~/.local/bin` (`--bin-output <folder>` for another); `unexport` takes it back |

It lists and touches only the containers it made: Distrobox containers of your own are left alone. On
Arch, `update` only refreshes the package lists, and Arch does not support installing from lists newer than
the packages already there: run `upgrade`, which refreshes them first.

## A package file

Double-click a `.deb` or an `.rpm` in Files, or:

```bash
amepkg install ~/Downloads/some-app.deb
```

A `.deb` goes into a Debian container, an `.rpm` into a Fedora one and a `.pkg.tar.zst` into an Arch one,
made from its template if there is none yet.

### What it checks first

- **A virus scan.** Nothing is installed if the file matches known malware. With the virus scanner turned
  off, it says the file was not scanned.
- **What the file says about itself:** its name and version, who made it, whether it is signed, and whether
  it runs scripts of its own as it installs. An `.rpm` can carry a signature, and one whose signature does
  not check out is refused. A `.deb` normally carries none, because Debian signs its repositories rather
  than each file, so a `.deb` from a website carries no proof of who made it.
- **Better sources first.** If the same app is on Flathub, or the package is in the container's own
  repositories, those are offered first: both are signed and kept up to date, and a file from a website is
  neither.

A package installed from a file is upgraded with its container only if its maker's repository comes with
it, as some vendors' packages set up as they install.

## Keeping them up to date

Every container is upgraded once a day by a timer of your own account (`amethystora-pkg-upgrade.timer`).
Like the system's automatic updates, it waits while the machine runs on a low battery, is busy, or is short
of memory, and tries again the next day; started by hand, it never waits. Only one runs at a time, and it
ends with an error when any container failed.

**Update now** in [System Updates](updates.md#updating-now) upgrades them too, straight after the system,
and shows how it went under **Containers**: a container that did not upgrade never makes the system's
update a failed one.

```bash
amepkg upgrade-all
journalctl --user -t amethystora-pkg-upgrade    # what each container said
```

The [security report](security.md#see-where-you-stand) says so when a container has gone two weeks without
an upgrade, and the **Containers** page of the Security app lists what is installed in each.

## Templates and package managers

A template names the image a container starts from, the package manager it is handled with, and the
packages each new container starts with. A package manager is the ten commands `amepkg` runs for it, from
`install` to `clean`. Make your own:

```bash
amepkg templates new --name devbox --base quay.io/toolbx/ubuntu-toolbox:24.04 \
    --pkg-manager apt --packages "build-essential git"
amepkg templates list
amepkg managers list
```

### Options of a template

- `--own-home` gives its containers a home folder of their own, which keeps their settings apart from
  yours. It does not hide your files: they stay reachable by their full path.
- distrobox's `--unshare` options (`--unshare netns,ipc`) suit command-line tools; an app with a window
  usually needs what they take away.

### Yours and the built-in ones

Yours are kept in `~/.local/share/amethystora/pkg`; the built-in ones cannot be changed or removed.
`show`, `update` and `rm` work on yours, as in `amepkg templates rm devbox`. `export` writes one to a file
and `import` reads it back, here or on another machine (`--output` names the file, `--name` keeps it under
another name). `managers` takes the same commands, and `managers new` takes each of the ten commands as a
flag, `--install` to `--clean`, with `--need-sudo` and `--noconfirm`.

### Import only what you would run

A definition is a program: a package manager's commands run in every container made with it, and a
template chooses the image they start from. `import` shows what the file defines and asks before it keeps
it, and `ame restore-setup` does the same for each one a backup brings. `import` also reads the YAML files
Apx writes for its stacks and package managers.

### Pinned and checked images

The built-in templates' images are pinned to a digest that ships inside the signed system image, so a
container starts from exactly the image Amethystora was built and tested with. The Debian, Alpine,
openSUSE Tumbleweed and openSUSE Leap images are also checked against their publishers' signatures: Leap's
against openSUSE's own keys, which check any Leap image you pull too.

The image of a template of your own is taken as its registry hands it out: `templates list` marks it
unverified, and making a container from it asks first, unless it is pinned with `@sha256:…` or comes from a
registry whose signatures this machine checks.

## The AUR

The Arch User Repository is off. Turn it on with the first line, or the switch on [Control](control.md)'s
**Apps** page, and make a container for it with the second:

```bash
ame apps aur on
amepkg containers new --template arch-aur
```

That adds the `arch-aur` template, whose containers get a home folder of their own, and the `paru` package
manager, built from the AUR in the container the first time.

### Before anything is built

Anyone can upload to the AUR and nobody reviews it, so `amepkg` first shows what the AUR says of a package:
who maintains it, how many vote for it, when it was submitted and last updated, and whether it is flagged
out of date, with a warning for one less than 30 days old, with no votes, or that nobody maintains. paru
then shows the PKGBUILD, the script that builds it: read it. What Arch's own repositories have comes from
them first.

The daily upgrade leaves what came from the AUR alone, because each new version is a new script to read:
`amepkg <name> upgrade`, in a terminal, upgrades those too.

## In scripts

The lists print JSON whose fields stay put, with `--json`:

- `amepkg containers list --json`: `name`, `container` (its name in podman), `template`, `manager`,
  `image`, `status`, `home`, `init`, `unshare`, `exported_apps`, `exported_bins`, `packages`, `files`,
  `aur_packages`, `aur_pending`, `created`, `last_upgrade` (both in Unix seconds) and
  `last_upgrade_result` (`ok`, `failed` or `null`); `home` is `null` for a container that shares yours, and
  `exported_apps` a list of `name` and `file`
- `amepkg templates list --json`: `name`, `description`, `base`, `manager`, `packages`, `own_home`,
  `unshare`, `built_in`, `verified` and `pinned`
- `amepkg managers list --json`: `name`, `need_sudo`, `noconfirm`, the ten commands and `built_in`

`containers new`, `templates new` and `update`, and `managers new` and `update` ask on a terminal for
whatever they were not given; with `--no-prompt`, they take flags only and fail on what is missing.
Scripts, launchers and services call it `amethystora-pkg`, its full name.

## Containers by hand

Distrobox and Toolbox work as they do anywhere:

```bash
distrobox create --name ubuntu --image quay.io/toolbx/ubuntu-toolbox:24.04
distrobox enter ubuntu
```

**DistroShelf** shows every container, yours and `amepkg`'s, in a window, and Ptyxis (`ptyxis`, see
[The terminal](terminal.md#opening-one)) opens a tab inside any of them from the menu next to its new tab
button. `distrobox-export --app <name>`, run inside
one, puts its app in the app grid.
