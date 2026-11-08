# Installing software

There is no `dnf install` here, and you will not miss it. Each kind of software has its own place, none of
them changes the system image, and so no app can break an update or be broken by one.

| You want | Use | How |
| --- | --- | --- |
| An app with a window | Flatpak, from Flathub | Bazaar, the software centre |
| A command line tool | Homebrew | `brew install <name>` |
| A whole Linux userland: `apt`, `dnf`, `pacman`, a toolchain | A container | `amepkg` ([Containers](containers.md)) |
| A `.deb` or an `.rpm` from a website | A container made for it | Open it in Files ([A package file](containers.md#a-package-file)) |
| A website as an app | A web app | `amethystora-webapp install` ([Web apps](web-apps.md)) |
| A Windows program | The same app for Linux first, else Bottles | Open it in Files ([Windows programs](windows-programs.md)) |
| Something that must be on the host itself | Layering | `rpm-ostree install`, as a last resort |

## Apps: Bazaar and Flathub

Open **Bazaar** and install from [Flathub](https://flathub.org), where nearly every Linux desktop app is
published. Flatpak apps update with the system, and removing one leaves nothing behind.

![Install from Bazaar](media/bazaar-install.webm)

```bash
flatpak install flathub org.gimp.GIMP
flatpak list --app
flatpak uninstall --delete-data org.gimp.GIMP
```

Right-click a Flatpak app in the app grid: **Uninstall** says what it leaves behind, and its details open
its page in Bazaar.

### The apps it comes with

They install from Flathub the first time the machine is online; Firefox is the one in the image itself.
Mail is Thunderbird; pictures open in Loupe, documents in Papers, videos in Showtime and sound in Decibels;
Snapshot is the camera, Gradia annotates screenshots and Pinta paints; Calculator, Calendar, Clocks,
Contacts, Maps, Weather, Characters, Fonts, Text Editor and File Roller for archives; Connections for remote
desktops, Document Scanner, Disk Usage Analyzer, Firmware, Mission Center, Impression to write an ISO to a
USB stick, Ignition for what starts at login, Metadata Cleaner, Smile for emoji, Refine for GNOME's hidden
settings, Extension Manager, DistroShelf and Bazaar itself. The
[developer image](developer.md#what-it-adds) adds its own.

### Removing an app

**Uninstall** says when you last opened the app and how much it keeps in its own folder, and takes that
too, unless you say otherwise.

- **The folder goes to the trash, not away.** **Undo** on the notice, or `ame apps undo-uninstall ID` until
  the trash is emptied (30 days as the image ships), installs the app again and puts it back. What it was
  granted beyond what it asks for itself goes with it, and it asks again.
- **Your documents are never part of it,** unless an app saved one only inside its own folder: look before
  you remove an app that keeps its work there.
- `ame desktop uninstall-data off`, or **Uninstall takes an app's data too** in [Control](control.md),
  brings back the question of whether to keep or delete the data.

`ame apps unused`, or **Apps you have not opened for months** in Control, lists the apps not opened for 90
days, with what each keeps, and removes those you pick the same way.

### Flatseal, Warehouse and stricter installing

- **Flatseal** shows and changes what each app may reach: folders, devices, the network.
- **Warehouse** manages installed apps and cleans up what they leave behind.
- `ame security flathub-verified on` shows only the Flathub apps whose makers Flathub verified, and
  `ame security flatpak-password on` asks for an administrator's password to install or remove an app for
  the whole machine. Both are off until you turn them on
  ([Stricter settings](security-switches.md#stricter-settings)).

### What Flatpak apps may not do

Every Flatpak is refused X11, `/dev/input` and the Flatpak service itself, whatever its own manifest asks
for: together those are how one app could read what you type into another. Apps written for Wayland do not
notice. An app that only speaks X11 (Wine, some games, some older Electron builds) needs it granted back,
one app at a time, in Flatseal or with:

```bash
flatpak override --user --socket=x11 <app-id>
```

The apps Amethystora installs that cannot work without the Flatpak service have it given back: Mission
Center (which reads the machine through a helper outside its sandbox), Bazaar, Warehouse, Refine,
DistroShelf, and on the developer image Podman Desktop and Builder.

What each app can reach beyond its sandbox, what it asked for and what you granted, is on the **Apps** page
of the Security app, and `ame security apps` in a terminal ([App permissions](app-permissions.md)).
**What can it reach?** in an app's menu in the app grid opens that page on the app.

### What starts with you

Control's **Apps** page, under **Starting with you**, lists every program that starts when you log in: the
autostart entries apps and you put in `~/.config/autostart`, those that came with the system, and your
account's own services. Each says who put it there: **The app**, **You**, **The system**, **Amethystora** or
**Unknown** (an entry whose program is not installed).

- **Off** stops it starting from the next login, and uninstalls nothing. An autostart entry gets a copy
  under its name in `~/.config/autostart` that only says `Hidden=true`, GNOME's own way to turn one off,
  with your own entry kept aside in `~/.local/share/amethystora/startup-off`; a Flatpak app is also refused
  running in the background, through its portal permission; a service of yours is disabled.
- **On** puts back what was there. Nothing is deleted.
- Amethystora's own (the phone link, the containers' upgrade, the backup...) are listed, and switched where
  each is set.

The [security watcher](security.md) reads every new autostart entry: one that only says `Hidden=true` starts
nothing, so it is recorded and not said; anything more there is said as before.

```bash
ame apps startup                        # what starts, who put it there, and its ID
ame apps startup off autostart:org.example.Chat.desktop   # on brings it back
```

## Command line tools: Homebrew

Homebrew installs into its own prefix, without `sudo`, and is updated along with the system. The
[terminal](terminal.md#command-line-tools) page has more.

```bash
brew install ripgrep fd bat
brew search <name>
```

### Sets to pick from

The image carries a Brewfile each in `/usr/share/amethystora/homebrew`: AI tools, wallpapers, the
terminal's tools, CNCF projects, fonts, a full desktop of GNOME Circle and other Flathub apps, editors,
Kubernetes tools and Swift. `ame apps bbrew` asks which and opens it in Bold Brew, where you choose what to
install, and `ame apps cncf` opens the CNCF one at once. Bold Brew comes from its author's own Homebrew
tap, which both ask before installing.

### Checks and analytics

`ame security brew-attestations on` has Homebrew check each bottle's build attestation before installing
it, which needs `gh` and `gh auth login` on every account. Homebrew's analytics, which it sends unless told
not to, are off (`/etc/homebrew/brew.env`).

## Layering: the last resort

`rpm-ostree install` adds a package to the image on this machine. Use it only for what cannot live anywhere
else, such as a driver or a system service, then restart to use it:

```bash
sudo rpm-ostree install <package>
```

Every layered package makes every update slower, and is kept only while you update through `rpm-ostree`.
Its updates come with the system's, and System Updates lists a newer version under **Waiting to be
installed** ([what is waiting](updates.md#what-is-waiting)).

- **Switching images or streams:** `bootc switch` builds the new system from the image alone and drops
  layered packages without a word. On a machine with layered packages, switch with
  `sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/<image>:<stream>` instead.
  Automatic updates handle this for you.
- **Kernel arguments:** `rpm-ostree` leaves out the ones the image sets, and Amethystora adds them to each
  update it stages ([updates](updates.md#what-is-waiting)), so they still change with the image.
- **Undoing it:** `sudo rpm-ostree uninstall <package>` removes one, and `sudo rpm-ostree reset` goes back
  to the plain image.

Adding a `dnf` repository or running `dnf install` on the host does not work: the system has no writable
package database.

## VPN

None of them runs until a connection is up. A VPN that will not connect: ask the agent, or see
`journalctl --no-hostname _COMM=nm-l2tp-service _COMM=pluto _COMM=kl2tpd _COMM=pppd`.

### Tailscale

Tailscale is installed. `ame system tailscale on` starts it and joins your tailnet, `ame system tailscale
off` stops it, and `ame system tailscale` alone says whether it is on; anything after `on` goes to
`tailscale up`, such as `--accept-routes`.

- **It runs only when used:** its service starts only once you turned it on, or where the machine had
  joined a tailnet before, so a machine that never uses it saves its memory.
- **It asks for your password** because Tailscale runs as root: whoever controls it can join the machine to
  a tailnet somebody else runs and turn on Tailscale SSH, which that tailnet can open to root.
  `sudo tailscale set --operator=$USER` lets your account run it without `sudo`, which lets any program you
  run do the same.
- **No logs are sent** to Tailscale by default; their support may ask for them, which
  `TS_NO_LOGS_NO_SUPPORT=false` in `/etc/default/tailscaled` turns back on.
- The firewall trusts the tailnet, where your tailnet's own access rules apply.

### WireGuard

WireGuard configurations from any provider can be imported in **Settings → Network → VPN**.

### A VPN from work or school

Add it in **Settings → Network → VPN**: the **+** button, or **Import from file…** for a `.ovpn` or a file
your administrator sent. The kinds the image can add:

- **OpenVPN**: import the `.ovpn` file.
- **OpenConnect** speaks Cisco AnyConnect, Palo Alto GlobalProtect, Pulse and Ivanti, Juniper, F5 and
  Array, and Fortinet; choose the protocol in the dialog. Upstream calls every one but AnyConnect
  experimental.
- **Fortinet SSL VPN (openfortivpn)**, a client of its own, for a FortiGate that OpenConnect's Fortinet
  protocol cannot reach (a realm, or a certificate to pin). Fedora 45 retired it, so an image built on 45
  reaches a FortiGate through OpenConnect only.
- **SSTP**: Microsoft's VPN over HTTPS.
- **IPsec based VPN**: IPsec with IKEv2, certificates included, through Libreswan.
- **Layer 2 Tunneling Protocol (L2TP)**, with IPsec, for older servers and routers. Libreswan refuses the
  old IKEv1 these usually speak ("global ikev1-policy does not allow IKEv1 connections"), as it should by
  default. If your server offers nothing else,
  `sudo sed -i 's/^#ikev1-policy=accept/ikev1-policy=accept/' /etc/ipsec.conf` allows it for this machine;
  undo it by putting the `#` back.
- **Cisco** `vpnc` (IPsec, with a `.pcf` file), up to Fedora 44, the last to have it.
- **PPTP is not offered.** Its login (MS-CHAPv2) can be broken from one recorded connection, whatever the
  password, and Microsoft's own servers are dropping it.

`ame security kernel strict` stops IPsec and L2TP/IPsec VPNs (it blocks the kernel's IPsec modules, see
[Stricter settings](security-switches.md#stricter-settings)); the others, WireGuard and Tailscale included,
are not affected.
