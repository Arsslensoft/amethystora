# Installing software

There is no `dnf install` here, and you will not miss it. Each kind of software has its own place,
none of them changes the system image, and so no app can ever break an update or be broken by one.

| You want | Use | How |
| --- | --- | --- |
| An app with a window | Flatpak, from Flathub | Bazaar, the software centre |
| A command line tool | Homebrew | `brew install <name>` |
| A whole Linux userland: `apt`, `dnf`, `pacman`, a toolchain | A container | `amepkg` ([Containers](#containers)) |
| A `.deb` or an `.rpm` from a website | A container made for it | Open it in Files ([A package file](#a-package-file)) |
| A website as an app | A web app | `amethystora-webapp install` ([Web apps](web-apps.md)) |
| A Windows program | The same app for Linux first, else Bottles | Open it in Files ([Windows programs](#windows-programs)) |
| Something that must be on the host itself | Layering | `rpm-ostree install`, as a last resort |

## Apps: Bazaar and Flathub

Open **Bazaar** and install from [Flathub](https://flathub.org), where nearly every Linux desktop app
is published. Flatpak apps update themselves along with the system, and removing one leaves nothing
behind. From a terminal:

```bash
flatpak install flathub org.gimp.GIMP
flatpak list --app
flatpak uninstall --delete-data org.gimp.GIMP
```

Or right-click a Flatpak app in the app grid: **Uninstall** says what it leaves behind, and its
details open its page in Bazaar.

The apps the machine comes with install from Flathub the first time it is online: Firefox is the one
in the image itself. Mail is Thunderbird; pictures open in Loupe, documents in Papers, videos in
Showtime and sound in Decibels; Snapshot is the camera, Gradia annotates screenshots and Pinta paints;
Calculator, Calendar, Clocks, Contacts, Maps, Weather, Characters, Fonts, Text Editor and File Roller
for archives; Connections for remote desktops, Document Scanner, Disk Usage Analyzer, Firmware, Mission
Center, Impression to write an ISO to a USB stick, Ignition for what starts at login, Metadata Cleaner,
Smile for emoji, Refine for GNOME's hidden settings, Extension Manager, DistroShelf and Bazaar itself.
The [developer image](developer.md#what-it-adds) adds its own.

### Removing an app

**Uninstall** says when you last opened the app and how much it keeps in its own folder, and takes that too, unless you say
otherwise. The folder goes to the trash, not away: **Undo** on the notice, or `ame apps undo-uninstall ID` until the trash is
emptied (30 days as the image ships), installs the app again and puts it back; what it was granted beyond what it asks for
itself goes with it, and it asks again. Your documents are never part of it, unless an
app saved one only inside its own folder, so look before you remove an app that keeps its work there. `ame desktop uninstall-data
off`, or **Uninstall takes an app's data too** in [Control](control.md), brings back the question of whether to keep or delete
the data.

`ame apps unused`, or **Apps you have not opened for months** in Control, lists the apps not opened for 90 days, with what each
keeps, and removes those you pick the same way.

Two more tools are there for Flatpaks:

- **Flatseal** shows and changes what each app is allowed to reach: folders, devices, the network.
- **Warehouse** manages installed apps and cleans up what they leave behind.

Two switches make installing stricter, each off until you turn it on:
`ame security flathub-verified on` shows only the Flathub apps whose makers Flathub verified, and
`ame security flatpak-password on` asks for an administrator's password to install or remove an app
for the whole machine ([Stricter settings](security.md#stricter-settings)).

### What Flatpak apps may not do

Every Flatpak is refused X11, `/dev/input` and the Flatpak service itself, whatever its own manifest
asks for. Together those are how one app could read what you type into another. Apps written for
Wayland do not notice. An app that only speaks X11 (Wine, some games, some older Electron builds)
needs it granted back, one app at a time, in Flatseal or with:

```bash
flatpak override --user --socket=x11 <app-id>
```

The apps Amethystora installs that cannot work without the Flatpak service have it given back:
Mission Center, which reads the machine through a helper outside its sandbox, Bazaar, Warehouse,
Refine, DistroShelf, and on the developer image Podman Desktop and Builder.

What each app can reach beyond its sandbox, what it asked for and what you granted it, is on the
**Apps** page of the Security app, and in a terminal with `ame security apps`
([App permissions](security.md#app-permissions)). **What can it reach?** in an app's menu in the app grid opens that
page on the app.

### What starts with you

Control's **Apps** page, under **Starting with you**, lists every program that starts when you log in: the autostart
entries apps and you put in `~/.config/autostart`, those that came with the system, and your account's own services,
each with who put it there (**The app**, **You**, **The system**, **Amethystora** or **Unknown**, for an entry whose
program is not installed). **Off** stops it starting from the next login, and does not uninstall it: an autostart entry
gets a copy under its name in `~/.config/autostart` that only says `Hidden=true`, the way GNOME has always let an account
turn one off, with your own entry kept aside in `~/.local/share/amethystora/startup-off`; a Flatpak app is also refused
running in the background, through its portal permission; a service of yours is disabled. **On** puts back what was
there. Nothing is deleted. Amethystora's own (the phone link, the containers' upgrade, the backup...) are listed, and
switched where each is set.

The [security watcher](security.md) reads every new autostart entry: one that only says `Hidden=true` starts nothing,
so it is recorded and not said; anything more there is said as before.

```bash
ame apps startup                        # what starts, who put it there, and its ID
ame apps startup off autostart:org.example.Chat.desktop   # on brings it back
```

## Command line tools: Homebrew

```bash
brew install ripgrep fd bat
brew search <name>
```

Homebrew installs into its own prefix, without `sudo`, and is updated along with the system. The
[terminal](terminal.md#command-line-tools) page has more.

The image carries sets to pick from, a Brewfile each in `/usr/share/amethystora/homebrew`: AI tools,
wallpapers, the terminal's tools, CNCF projects, fonts, a full desktop of GNOME Circle and other
Flathub apps, editors, Kubernetes tools and Swift. `ame apps bbrew` asks which and opens it in Bold
Brew, where you choose what to install from it, and `ame apps cncf` opens the CNCF one at once. Bold
Brew comes from its author's own Homebrew tap, which both ask before installing.

`ame security brew-attestations on` has it
check each bottle's build attestation before installing it, which needs `gh` and `gh auth login` on
every account. Homebrew's analytics, which it sends unless told not to, are off
(`/etc/homebrew/brew.env`).

## Containers

For anything that expects a traditional distribution (a compiler toolchain, a `.deb` a vendor ships,
a package only on the Arch User Repository, an Ubuntu-only tool), make a container. It shares your
home folder and your display, so apps inside it open windows like any other, and it can be thrown away
without a trace. Nothing installed in one touches the system.

> **A container is not a sandbox.** What you install in one runs as you, with your home folder, your
> display, your sound and your session bus. When an app is on Flathub, have it from there: Flatpak
> apps are sandboxed, signed and updated with the system.

### Packages from other distributions

`amepkg`, short for `amethystora-pkg` and also `ame pkg`, makes containers from templates (`debian`,
`ubuntu`, `fedora`, `arch`, `alpine`, `opensuse-tumbleweed` and `opensuse-leap`) and installs into them with their own package managers:

```bash
amepkg containers new --template debian
amepkg debian install gimp
amepkg debian enter
```

The first line downloads the image the template names, makes the container and brings it up to
date. `install` installs from Debian's own signed repositories with `apt`, and puts every app the
package brought with it in the app grid, marked "(on debian)"; `remove` takes them out again. Before
it installs an app, `amepkg` looks for the same app on Flathub and offers that first.

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
Arch, `update` only refreshes the package lists, and Arch does not support installing from lists
newer than the packages already there: `upgrade`, which refreshes them first, is the one to run.

### A package file

Double-click a `.deb` or an `.rpm` in Files, or from a terminal:

```bash
amepkg install ~/Downloads/some-app.deb
```

A `.deb` goes into a Debian container, an `.rpm` into a Fedora one and a `.pkg.tar.zst` into an Arch
one, made from its template if there is none yet. First the file is scanned for viruses, and nothing
is installed if it matches known malware. Then it says what the file says about itself: its name and
version, who made it, whether it is signed, and whether it runs scripts of its own as it installs. An
`.rpm` can carry a signature; a `.deb` normally does not, because Debian signs its repositories rather
than each file, so a `.deb` from a website carries no proof of who made it. If the same app is on
Flathub, or the package is in the container's own repositories, those are offered first: both are
signed and kept up to date, and a file from a website is neither. With the virus scanner turned off it
says the file was not scanned, and an `.rpm` whose signature does not check out is refused.

A package installed from a file is upgraded with its container only if its maker's repository comes
with it, as some vendors' packages set up as they install.

### Keeping them up to date

Every container is upgraded once a day by a timer of your own account
(`amethystora-pkg-upgrade.timer`). Like the automatic updates of the system, it waits while the
machine runs on a low battery, is busy, or is short of memory, and tries again the next day. Started
by hand, it never waits. Only one runs at a time, and it ends with an error when any container failed. **Update now** in
[System Updates](updates.md#updating-now) upgrades them too, straight after the system, and shows how
it went under **Containers**: a container that did not upgrade never makes the system's update a
failed one. In a terminal:

```bash
amepkg upgrade-all
journalctl --user -t amethystora-pkg-upgrade    # what each container said
```

The [security report](security.md#see-where-you-stand) says so when a container has gone two weeks
without an upgrade, and the **Containers** page of the Security app lists what is installed in each.

### Templates and package managers

A template names the image a container starts from, the package manager it is handled with, and the
packages each new container starts with. A package manager is the ten commands `amepkg` runs for it,
from `install` to `clean`. Make your own:

```bash
amepkg templates new --name devbox --base quay.io/toolbx/ubuntu-toolbox:24.04 \
    --pkg-manager apt --packages "build-essential git"
amepkg templates list
amepkg managers list
```

A template can give its containers a home folder of their own (`--own-home`), which keeps their
settings apart from yours, and distrobox's `--unshare` options (`--unshare netns,ipc`), which suit
command-line tools; an app with a window usually needs what they take away. A home folder of its own
does not hide your files: they stay reachable by their full path. Yours are kept in
`~/.local/share/amethystora/pkg`, and the built-in ones cannot be changed or removed: `show`, `update`
and `rm` work on yours, as in `amepkg templates rm devbox`. `export` writes
one to a file, and `import` reads it back, here or on another machine (`--output` names the file, `--name` keeps it
under another name). `managers` takes the same commands, and `managers new` takes each of the ten
commands as a flag, `--install` to `--clean`, with `--need-sudo` and `--noconfirm`. A definition is a program: a
package manager's commands run in every container made with it, and a template chooses the image they
start from, so import only what you would run yourself: `import` shows what the file defines and asks
before it keeps it, and `ame restore-setup` does the same for each one a backup brings.
`import` also reads the YAML files Apx writes for its stacks and package managers.

The built-in templates' images are pinned to a digest that ships inside the signed system image, so
a container starts from exactly the image Amethystora was built and tested with, and the Debian,
Alpine, openSUSE Tumbleweed and openSUSE Leap images are checked against their publishers'
signatures as well: Leap's against openSUSE's own keys, which check any Leap image you pull too. The
image of a template of your own is taken as its registry hands it out: `templates list` marks it
unverified, and making a container from it asks first, unless it is pinned with `@sha256:…` or comes
from a registry whose signatures this machine checks.

### The AUR

The Arch User Repository is off. Turn it on with the first of these, or the switch on
[Control](control.md)'s **Apps** page, and make a container for it with the second:

```bash
ame apps aur on
amepkg containers new --template arch-aur
```

That adds the `arch-aur` template, whose containers get a home folder of their own, and the `paru`
package manager, which is built from the AUR in the container the first time. Anyone can upload to
the AUR and nobody reviews what is there, so before anything is built `amepkg` shows what the AUR
says of it: who maintains it, how many vote for it, when it was submitted and last updated, and
whether it is flagged out of date, with a warning for one less than 30 days old, with no votes, or
that nobody maintains.
paru then shows you the PKGBUILD, the script that builds it: read it. What Arch's own repositories
have comes from them first. The daily upgrade leaves what came from the AUR alone, because each new
version is a new script to read: `amepkg <name> upgrade`, in a terminal, upgrades those too.

### In scripts

The lists print JSON whose fields stay put, with `--json`:

- `amepkg containers list --json`: `name`, `container` (its name in podman), `template`, `manager`,
  `image`, `status`, `home`, `init`, `unshare`, `exported_apps`, `exported_bins`, `packages`,
  `files`, `aur_packages`, `aur_pending`, `created`, `last_upgrade` (both in Unix seconds) and
  `last_upgrade_result` (`ok`, `failed` or `null`); `home` is `null` for a container that shares
  yours, and `exported_apps` a list of `name` and `file`
- `amepkg templates list --json`: `name`, `description`, `base`, `manager`, `packages`, `own_home`,
  `unshare`, `built_in`, `verified` and `pinned`
- `amepkg managers list --json`: `name`, `need_sudo`, `noconfirm`, the ten commands and `built_in`

`containers new`, `templates new` and `update`, and `managers new` and `update` ask on a terminal for
whatever they were not given. With `--no-prompt`, they take flags only and fail on what is missing.
Scripts, launchers and services call it `amethystora-pkg`, its full name.

### Containers by hand

Distrobox and Toolbox work as they do anywhere:

```bash
distrobox create --name ubuntu --image quay.io/toolbx/ubuntu-toolbox:24.04
distrobox enter ubuntu
```

**DistroShelf** shows every container, yours and `amepkg`'s, in a window, and the terminal opens a
tab inside any of them from the menu next to its new tab button. An app installed in one can be put
in the app grid from inside it with `distrobox-export --app <name>`.

## Windows programs

Double-click a Windows program in Files (an `.exe`, an `.msi`, a shortcut or a `.bat`) and a terminal opens that
asks what to do with it. First it scans the file for viruses and says who signed it: a name is only said as the
publisher's when the signature checks out, since anyone can sign a program in any name. Nothing is run if it
matches known malware, or if it was changed after it was signed.

Then, for some sixty well-known programs, it offers better ways to the same thing, before the Windows program:

- **The same app for Linux**, from Flathub: Spotify, Discord, Zoom, Slack, VLC, OBS and many more. It updates with
  the system and runs in its sandbox, and opens straight away if it is installed already.
- **An app that does the same job**: LibreOffice or ONLYOFFICE for Microsoft Office, Pinta for Paint.NET, File
  Roller for 7-Zip and WinRAR, Papers for Acrobat Reader, GIMP, Krita and Inkscape for Adobe's apps.
- **Its web version, as a [web app](web-apps.md)**: Microsoft Teams, WhatsApp, Office, Google Drive.
- **Steam and the game launchers** are set up by `ame apps gaming` ([Games](games.md)), where Windows games run
  through Proton.

Otherwise, or if you want the Windows program itself, **Run the Windows program, with Bottles** installs
[Bottles](https://usebottles.com) from Flathub, makes a bottle named Windows the first time, and hands it the
file. Bottles runs Windows programs with Wine, and asks which bottle each one goes in. Wine draws through X11,
which every app is refused here (an app on X11 can read what you type into other X11 apps), so X11 is granted
back to Bottles alone, for your account: the Security app's **Apps** page lists it, and
`flatpak override --user --reset com.usebottles.bottles` takes it back. Bottles only sees the file you opened,
not your home folder: a program that needs the files next to it (an installer unpacked from a CD) has to be
opened from Bottles itself, after you give it that folder in Flatseal.

Some programs never run with Wine: games whose anti-cheat runs in the Windows kernel (Valorant, League of
Legends, FACEIT), Microsoft 365's desktop apps and Adobe's Creative Cloud. The handler says so, as it does for a
Windows driver: Linux brings its own, and `ame system drivers` says which one each device uses. For the
rest, a Windows virtual machine runs nearly everything: the [developer image](developer.md) comes with virtual
machines, and WinBoat, which you install yourself, shows Windows apps from one as windows of their own (the
Windows licence is yours to bring). A package from the Microsoft Store (`.msix`, `.appx`) is only matched
against the list, since Wine cannot install it.

To find what replaces a program the list does not know, [Flathub](https://flathub.org) and
[AlternativeTo](https://alternativeto.net) are the places to look.

## Layering: the last resort

`rpm-ostree install` adds a package to the image on this machine. Use it only for what cannot live
anywhere else, such as a driver or a system service:

```bash
sudo rpm-ostree install <package>
```

Restart to use it. Every layered package makes every update slower, and the package is only kept
while you update through `rpm-ostree`. Two things to know:

- **Switching images or streams:** `bootc switch` builds the new system from the image alone and
  drops layered packages without a word. On a machine with layered packages, switch with
  `sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/<image>:<stream>` instead.
  Automatic updates handle this for you.
- **Undoing it:** `sudo rpm-ostree uninstall <package>` removes one, and `sudo rpm-ostree reset`
  goes back to the plain image.

Adding a `dnf` repository or running `dnf install` on the host does not work: the system has no
writable package database.

## VPN

- **Tailscale** is installed: `ame system tailscale on` starts it and joins your tailnet, and
  `ame system tailscale off` stops it; anything after `on` goes to `tailscale up`, such as
  `--accept-routes`, and `ame system tailscale` alone says whether it is on. Its service runs only once you turned it on, or where the machine
  had joined a tailnet before, so a machine that never uses it saves its memory. Turning it on asks for
  your password because
  Tailscale runs as root, and whoever controls it can join the machine to a tailnet somebody else runs
  and turn on Tailscale SSH, which that tailnet can open to root. `sudo tailscale set --operator=$USER`
  lets your account run it without `sudo`, which lets any program you run do the same. It sends
  Tailscale none of the logs it would by default; their support may ask for them, which
  `TS_NO_LOGS_NO_SUPPORT=false` in `/etc/default/tailscaled` turns back on.
  The firewall trusts the tailnet, where your tailnet's own access rules apply.
- **WireGuard** configurations from any provider can be imported in **Settings → Network → VPN**.
- **A VPN your work or school gave you** is added in **Settings → Network → VPN** (the **+** button, or
  **Import from file…** for a `.ovpn` or a file your administrator sent). The kinds the image can add:
  - **OpenVPN**: import the `.ovpn` file.
  - **OpenConnect** speaks Cisco AnyConnect, Palo Alto GlobalProtect, Pulse and Ivanti, Juniper, F5 and Array,
    and Fortinet; choose the protocol in the dialog. Upstream calls every one but AnyConnect experimental.
  - **Fortinet SSL VPN** has a client of its own, **Fortinet SSL VPN (openfortivpn)**, for a FortiGate that
    OpenConnect's Fortinet protocol cannot reach (a realm, or a certificate to pin). Fedora 45 retired it, so
    an image built on 45 reaches a FortiGate through OpenConnect only.
  - **SSTP** is Microsoft's VPN over HTTPS.
  - **IPsec based VPN** is IPsec with IKEv2, certificates included, through Libreswan.
  - **Layer 2 Tunneling Protocol (L2TP)**, with IPsec, for older servers and routers. Libreswan refuses the
    old IKEv1 these usually speak ("global ikev1-policy does not allow IKEv1 connections"), as it should by
    default. If your server offers nothing else, `sudo sed -i 's/^#ikev1-policy=accept/ikev1-policy=accept/' /etc/ipsec.conf`
    allows it for this machine; undo it by putting the `#` back.
  - **Cisco** `vpnc` (IPsec, with a `.pcf` file) is there too, up to Fedora 44, which is the last to have it.
  - **PPTP is not offered.** Its login (MS-CHAPv2) can be broken from one recorded connection, whatever the
    password, and Microsoft's own servers are dropping it.

  None of them runs until a connection is up. `ame security kernel strict` stops IPsec and L2TP/IPsec VPNs
  (it blocks the kernel's IPsec modules, see [Stricter settings](security.md#stricter-settings)); the others,
  WireGuard and Tailscale included, are not affected. A VPN that will not connect: ask the agent, or see
  `journalctl --no-hostname _COMM=nm-l2tp-service _COMM=pluto _COMM=kl2tpd _COMM=pppd`.
