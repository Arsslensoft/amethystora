# Amethystora

[![Stable Images](https://github.com/arsslensoft/amethystora/actions/workflows/build-image-stable.yml/badge.svg)](https://github.com/arsslensoft/amethystora/actions/workflows/build-image-stable.yml)[![Latest Images](https://github.com/arsslensoft/amethystora/actions/workflows/build-image-latest-main.yml/badge.svg)](https://github.com/arsslensoft/amethystora/actions/workflows/build-image-latest-main.yml)

**Amethystora** is a cloud-native desktop operating system built on Fedora, using container technology and atomic updates. Its desktop is [GNOME](https://www.gnome.org).

For end users, it aims to be as reliable as a Chromebook with near-zero maintenance. For developers, it offers a cloud-native workflow with integrated container tools, declarative system management and CI/CD-built images.

Amethystora is created and maintained by **Arsslen Idadi** ([@iarsslen](https://github.com/iarsslen)).

## Mission

Amethystora aims to be a robust, cloud-native desktop that brings enterprise-grade infrastructure practices to everyday computing:

- **Reliability**: Atomic updates keep the system stable
- **Developer Experience**: Integrated cloud-native tooling and workflows, including Kubernetes and container support
- **Sustainability**: Low maintenance overhead through modern cloud-native build infrastructure

## Images

| Image | Description |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | The standard desktop |
| `ghcr.io/arsslensoft/amethystora-dx` | Developer Experience: adds Docker, Incus, libvirt and developer tools |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | Standard desktop with the open NVIDIA kernel modules |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Developer Experience with the open NVIDIA kernel modules |
| `ghcr.io/arsslensoft/amethystora-nvidia-lts` | Standard desktop with NVIDIA's closed driver of its 580 branch, for the GeForce 900 and 10 series |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-lts` | Developer Experience with NVIDIA's closed driver of its 580 branch |

Each image is published in the `stable`, `latest` and `beta` streams.

Each stable image also comes sealed, as `stable-sealed` (experimental): its kernel, the kernel's settings and the digest of every file of the system are one image signed for Secure Boot, and a file changed on the disk cannot be read at all. A sealed system can only be installed fresh, from the live ISO's **Install Amethystora, sealed** launcher, never switched to; the manual's Installing page says what it changes.

Each image has its ISOs, named after it (`amethystora-dx.iso`, `amethystora-nvidia-lts-non-uefi.iso`). On a machine with an NVIDIA card installed without the driver that drives it, a notification a few minutes after the first boot offers to switch to the matching `-nvidia-open` image (Turing and newer) or `-nvidia-lts` image (Maxwell, Pascal and Volta) through `ame system rebase`, which takes signed images only, and with Secure Boot on only once the keys are enrolled; it never switches by itself. Without Secure Boot, the NVIDIA images run without kernel lockdown (see [Security](#security)).

## Getting Started

Download [amethystora.iso](https://download.amethystora.org/amethystora.iso), write it to a USB stick and start the computer from it: Amethystora runs from the stick, to try without changing anything on the computer, and installs from there, offline, beside Windows if you want to keep it. It needs UEFI firmware. The installer alone, which also starts older computers, is [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso). Each has its checksum beside it as `<name>.sha256`, signed with the key in `cosign.pub`; the manual's Installing page says how to check it and how to install next to Windows.

From an existing Fedora Atomic or Universal Blue system, switch to Amethystora with:

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Then reboot. Replace `amethystora` with any image name from the table above, and `stable` with the stream you want.

### Using the desktop

You log in on GDM to a GNOME session with the Amethystora wallpaper and a purple accent. These GNOME Shell extensions are preinstalled: Amethystora Logo Menu, Amethystora UI (the desktop's own modules, written for this image, among them the tiling and the phone link's top bar icon) and Amethystora Clipboard (a copy of Clipboard Indicator that seals its history). Turn them on or off in Extensions.

The top bar carries the six workspaces on the left, where Activities used to be, and CPU, memory and network meters on the right. Both follow the current theme.

#### The manual

Everything about using Amethystora is in the Amethystora Manual, which ships with the image and works offline: getting around, the hotkeys, themes, installing software, developer mode, updates and rollbacks, security, hardware and troubleshooting. Open it with `Super+F1`, from the app grid or the Amethystora menu, or with `amethystora-manual`; `amethystora-manual <page>` opens a page directly. The pages are Markdown in [system_files/shared/usr/share/amethystora/manual](system_files/shared/usr/share/amethystora/manual), and the hotkeys page is the same `keybindings.md` that `ame desktop keybindings` shows.

#### Languages

The Manual, Security, System Updates, Logs, Notes, Control and Backups, the security report and the manual's pages follow the session's language. They are translated into French, Arabic (right to left), Spanish and German, and into any other language people send; [CONTRIBUTING.md](CONTRIBUTING.md#translating) says how. A message without a translation yet shows in English.

#### System commands

`ame` runs the commands that come with the image, in groups: `ame` lists the groups, `ame security` lists the commands in that group, and `ame security scan now` runs one. `ame pkg` is `amethystora-pkg`. `ujust` runs the same commands, and still takes the names they had before they were grouped, such as `ujust setup-backup`.

#### Tiling and keyboard navigation

Windows tile on their own: each workspace is a strip of windows side by side that scrolls to keep the focused one in view. `Super` and an arrow moves between windows, `Super+Ctrl` and an arrow moves the window, `Super+R` steps its width and `Super+F` fills the screen. `Super+Shift+V` opens the clipboard history.

There are six fixed workspaces on `Super+1` to `Super+6`, `Super+Shift+N` takes the window with you, and `Ctrl+Alt+1` to `Ctrl+Alt+9` reach the pinned apps in the dock. `Super+Return` opens a terminal (kitty), `Super+W` closes a window, `Super+Space` searches.

`Super+Alt+Space` opens the Amethystora menu, which reaches all of this from the keyboard. The full list is in `ame desktop keybindings`.

Windows that float and overlap instead, as on Windows and macOS, are `ame desktop layout classic`, which updates leave alone; the first login offers it in a notification. `ame desktop layout tiling` brings the strip back.

#### Themes

`Super+Ctrl+Shift+Space` picks a theme, `Super+Ctrl+D` flips between light and dark, and `Super+Ctrl+Space` cycles the wallpaper (every theme has three wallpapers of made-up places drawn in its palette and opens on the first, the Amethystora themes open on a cut stone's facets instead and add the crystal field and a night sky, and pictures added to `~/.config/amethystora/backgrounds/<theme>/` join them). One switch repaints GNOME's colour scheme and accent, the Ptyxis palette, the shell prompt, btop, the dock and the wallpaper, because each theme is a single palette file everything else is rendered from. The same from a terminal:

```bash
ame desktop theme                # pick one
amethystora-theme set nord       # or name it
amethystora-theme list
```

Amethystora ships Amethystora (light and dark), Tokyo Night, Catppuccin Mocha and Latte, Gruvbox, Nord, Rosé Pine Dawn, Everforest and Matte Black. To add your own, drop a directory into `~/.config/amethystora/themes/` with a `colors.toml` in it; a directory that matches a shipped theme's name is layered over it, so you can change one colour without copying the rest. `/usr/lib/amethystora/theme/lib.sh` documents the other files a theme can hold.

The icons are three packs drawn for Amethystora by `branding/icons.mjs`, each icon in colours of its own: Glass (a dark theme), Duotone (a light theme) and Neon, chosen with `ame desktop icons`. The panel keeps GNOME's own monochrome icons so the meters and indicators stay legible. The packs draw Amethystora's apps, GNOME's, folders, file types, devices and generic pictograms; an app with a logo of its own, Firefox for one, keeps the icon its maker drew, as shipped, because a logo is a trademark and not ours to restyle. A theme that wants a different set names it in an `icons.theme` file.

#### AI agent

Every image is set up for [Claude Code](https://claude.com/claude-code), [Opencode](https://opencode.ai), [Codex](https://github.com/openai/codex), [Antigravity CLI](https://antigravity.google/product/antigravity-cli), [Copilot CLI](https://github.com/github/copilot-cli) and [Cursor CLI](https://cursor.com/cli), with a skill that explains this system to them: which paths belong to the image and which are yours to change, how themes and keybindings work, how to install software on an atomic system, and what to ask you before touching. `Super+Ctrl+Shift+A`, or "Ask an agent" in the Amethystora menu, opens one in a terminal in its normal mode, so it asks before it acts. "Diagnose a problem" in the same menu hands it a crash, a failed service or your own description, with a second skill that has it investigate from the logs and core dumps, change nothing, and come back with the cause and a proposed fix.

```bash
amethystora-agent ask "make the dock icons smaller"
amethystora-agent diagnose       # why did something break? Also: a PID, a service, or "wifi drops after suspend"
amethystora-agent use codex      # or opencode, agy, copilot, cursor-agent; the default is Claude Code
ame agent install                # install it ahead of time instead of on first launch
ame agent toggle                 # turn the agent features off, or on again
```

The agents themselves are not in the image, because they release far more often than it does. The first launch offers to install the chosen one into your home with its maker's own installer, and from then on it keeps itself up to date and you decide when: `claude update`, `opencode upgrade`, `codex update`, `agy update`, `copilot update` or `cursor-agent update`, or turn automatic updates off in their settings. Antigravity CLI took the place of Gemini CLI; a Gemini CLI already installed from Homebrew still opens and updates with it. Claude Code is installed on its stable channel, a release about a week old. Since it lives in your home, the same copy also runs inside any toolbox or distrobox.

Turning the features off removes the menu entry, disables the keybinding and unlinks the skills from `~/.claude/skills`, `~/.agents/skills` and `~/.gemini/antigravity-cli/skills`. An installed agent stays installed either way, and none does anything until you sign in to it.

Where something fails, the apps hand it over: **Ask the agent** in System Updates when an update did not finish, in Security beside anything that needs your attention, and in Logs beside a crash or a service's message. They pass on only the unit, the process number or the report's own words, and the agent reads the rest itself.

#### Software from other distributions

`amepkg`, short for `amethystora-pkg`, installs software packaged for other distributions into rootless containers made for it, from templates for Debian, Ubuntu, Fedora, Arch, Alpine and openSUSE: `amepkg containers new --template debian`, then `amepkg debian install <package>`, and the package's apps appear in the app grid. A downloaded `.deb` or `.rpm` opens in it from Files, after a virus scan and a look at where it comes from, with the same app from Flathub offered first. Each template's image is pinned by digest in the signed image, the containers are upgraded every day, and the Arch User Repository is off until `ame apps aur`. Containers are not a sandbox: what runs in one runs as you, with your home folder and your session, so Flatpak stays the place for apps with windows.

#### Games and Android apps

Each is one command that says what it changes before it does, and takes it all away again:

```bash
ame apps gaming        # Steam from Flathub, ProtonPlus, MangoHud and gamescope, with X11 granted to Steam alone
ame apps android       # Android apps in Waydroid, LineageOS without Google's apps
ame system scheduler   # a sched-ext CPU scheduler for games (lavd), a busy desktop (bpfland) or audio work (flash)
```

#### Boot menu

The GRUB menu can carry the Amethystora artwork too — the crystal field from the wallpapers, the wordmark above the entries, and the boot splash following on the same dark ground. It is off by default and turned on with:

```bash
ame desktop boot-menu
```

It is a separate step because GRUB reads the boot filesystem and `/boot` is not part of the image: the theme ships read-only in `/usr/share/grub/themes/amethystora` and is copied onto `/boot` on the machine. Settings go in `/boot/grub2/user.cfg` inside a marked block, leaving `grub.cfg` alone — `grub2-mkconfig` cannot regenerate it on Fedora Atomic, and bootupd rewrites it anyway. Once the theme is on, image updates refresh the artwork by themselves. `ame desktop boot-menu` turns it off again, and the machine falls back to the plain menu.

#### Security keys

A FIDO2 security key (YubiKey, Thetis, or a fingerprint model such as the YubiKey Bio or Thetis Bio) can sign you in, unlock the screen and approve `sudo` and administrator prompts in place of your password. Register one with `ame security key`. Choose "Add a fingerprint key" for a fingerprint model so that the key checks your fingerprint and not just a touch. The key needs a PIN and an enrolled fingerprint first; set these in Firefox at `about:webauthn`, or with `ykman fido fingerprints add` on a YubiKey Bio. Register a second key as a spare. Your password keeps working, and it is used whenever no registered key is plugged in.

A fingerprint reader works the same way, for the login screen, the lock screen, `sudo` and administrator prompts: enrol a finger with `ame security fingerprint`, or in Settings under Users. The disk passphrase at boot and the keyring still take the password.

### Building locally

```bash
just build amethystora latest main
just build amethystora-dx latest main
```

### Security

Amethystora hardens the Fedora defaults:

- **Updates must be signed.** Only images signed with Amethystora's key are accepted, so a tampered or substituted image is refused instead of installed. The first boot after this change switches an existing installation over automatically.
- **The firewall rejects incoming connections.** Fedora Workstation leaves every port above 1024 open to the local network; Amethystora only allows printer and device discovery (mDNS), Windows file sharing and IPv6 setup, and phone pairing only on the networks you name home (`ame security home-networks`). Tailscale is trusted, where your tailnet's access rules apply. To let something else in, for example a development server you want to open on your phone, use the Firewall app or `sudo firewall-cmd --add-port=8080/tcp` (add `--permanent` to keep it after a reboot).
- **Ten wrong passwords in a row** lock an account for ten minutes. Unlock it early from another administrator account with `sudo faillock --user <name> --reset`.
- **Too many failed logins from one address** and fail2ban has the firewall reject that address: five failures within ten minutes cost an hour, and each further ban of the same address lasts longer than the last, up to a day. It watches the SSH server, the only thing here that can be logged into over the network, and it leaves the tailnet alone. See what is blocked with `ame security addresses`, and let an address back in with `ame security addresses <address>`.
- **Kernel hardening**: memory is wiped as it is handed out, kernel addresses are hidden, programs cannot read each other's memory, and rarely used modules (old network protocols and filesystems, FireWire) cannot load; FireWire has a switch of its own, `ame security firewire`, for FireWire sound cards. `gdb -p` on a process you did not start now needs `sudo`.
- **The SSH server stays off** and refuses root logins when you turn it on.
- **Applications cannot watch each other's keyboards.** Every Flatpak is refused X11, `/dev/input` and the Flatpak service itself, whatever its own manifest asks for. X11 is the one way left on a Wayland desktop for one application to read what you type into another, and `/dev/input` is every key on the machine. Applications that speak Wayland are unaffected; X11-only ones (Wine, some Electron builds, older games) need it granted back. Flatseal is preinstalled for exactly that, one application at a time, or use `flatpak override --user --socket=x11 <application>`. The system-wide rules are in `/etc/flatpak/overrides/global`, which Flatpak reads through a link in `/var/lib/flatpak/overrides`. The Security app's **Apps** page, and `ame security apps`, show what each app can reach beyond its sandbox, and the report says when one can leave it.
- **Key remapping is off until you ask for it.** Input Remapper runs as root and reads every input device on the machine, which is every key typed into every application. That is what remapping a key needs, and it is a keylogger in every other respect, so it ships switched off. Turn it on with `ame security input-remapper` if you remap keys.
- **Block USB devices you did not plug in** with `ame security usb`. A USB device can claim to be a keyboard and type by itself, and a hardware keylogger can sit between a real keyboard and the machine. Either new devices are blocked while the screen is locked, plain keyboards and mice aside, or USBGuard allows what is connected when you set it up and blocks anything new until you allow it. It is off by default, because a machine that blocked its own keyboard on first boot would be unusable.
- **Keep a private folder that locks with the screen** with `ame security private-folder`: `~/Private`, encrypted on its own with gocryptfs and a password of its own, closed whenever the screen locks, before sleep and at logout, so that its key is out of memory while you are away. A recovery key is shown once when you make it.
- **Let browsers install only the extensions on a list** with `ame security browser`. A browser extension sees every page you open, and that is how most credential stealers arrive. Turned on, Firefox, Brave, Chrome, Chromium and Edge install only the password managers on Amethystora's list and the extensions you allow with `ame security browser allow <id>`, and web pages are cut off from USB, serial, HID and Bluetooth devices. Firefox removes extensions that are not on the list, so allow yours first. It is off by default, because the image ships Firefox exactly as Fedora builds it: the policy is written on your machine, when you ask for it. `ame security browser privacy` is its own switch, also off, that tells the browsers not to send usage data, studies or sponsored-content pings to their makers.
- **Back up somewhere ransomware cannot reach** with `ame backup`. Ransomware encrypts everything it can reach, and that includes a backup this machine can delete from. The daily restic backup only ever adds to its repository: it never runs `forget` or `prune`, so an append-only repository at the other end keeps what was backed up before the machine was taken over. The recipe explains how to set one up on rest-server, Borg, an object store with immutability, or a USB drive you unplug between backups. Thinning out old snapshots is done from somewhere else, with credentials this machine does not hold.
- **Snapshot the home folders every hour** with `ame security ransomware`, so that files ransomware encrypted can be put back as they were. The snapshots are read-only btrfs snapshots of `/var/home`: nothing running as a user can change or delete them, and each user sees only their own files in them. They are kept for a day, then one a day for two weeks, and never deleted to make room, because ransomware that rewrites every file would then delete the ones from before it. They cost disk space, so this is off by default, and they do not survive ransomware that gets root, which is what the backup above is for.
- **Get files back in a window**: **Backups**, in the app grid, shows the home folder as it was at any hour the snapshots or the backup kept, on one timeline, and puts back what you pick without deleting anything.
- **Unlock the disk with the TPM** instead of typing the passphrase at every boot, with `ame security disk-unlock`. The TPM hands over the key only while Secure Boot is on and the enrolled signing keys are the ones enrolled when you set it up, so turning Secure Boot off stops the disk opening. It does not measure the initramfs, which changes with every kernel update; add a PIN if you want the disk to need something you know as well. Your passphrase keeps working, so a cleared TPM never locks you out.
- **The kernel refuses to be rewritten while it runs.** Lockdown refuses a module the kernel does not trust, writing `/dev/mem`, and BPF that writes into another program's memory, so a root compromise ends at the next reboot instead of moving into the kernel where nothing can find it. With Secure Boot on, the kernel locks itself down at every start. Without it, a kernel argument does, but the modules built with the image (evdi for DisplayLink docks, xone and xpadneo for Xbox controllers) are signed with keys of the image's own and cannot load then: run `ame security secure-boot` and turn Secure Boot on, or, where that is not possible, `ame security lockdown off`. The NVIDIA images leave the argument out: their driver is signed the same way.
- **What would have to change to survive a reboot is watched, and you are told.** Audit records changes to accounts, to who may use sudo, to your autostart entries, your own systemd units and the shell startup files, to the security settings, and every attempt to open an input device. The security watcher reads it every 15 minutes, with the journal (accounts locked, wrong sudo passwords, blocked USB devices), `/etc` against the image's own copy in `/usr/etc`, and what should never exist on an image-based system: setuid programs outside `/usr`, a preloaded library, kernel modules the image did not ship, programs newly listening on the network. What is new is a notification; `ame security events` shows it.
- **A virus scan runs weekly** over home and temporary directories, and **Lynis audits the settings monthly**. By default neither deletes, quarantines or moves anything: a false positive that takes away a file you wanted is worse than most of what it would be removing. See what the last scan found, or run one now, with `ame security scan`.
- **Two settings, off by default,** in `ame security settings`: have scans quarantine (restorable with `ame security scan restore`) or delete what they find, and real-time watching, which reads each change as the audit log records it and scans every file written in a home folder as it is closed.
- **Check the machine over whenever you like** with `ame security audit`. Lynis goes through a few hundred checks and prints what it found together with what to do about each one; it reads the system and changes nothing. What it checks is set in `/etc/lynis/custom.prf`.

### Secure Boot

Secure Boot is supported. Amethystora uses the kernel modules from Universal Blue's akmods, which are signed with the Universal Blue key. After the first installation, you will be prompted to enroll the Secure Boot key in the BIOS.

Enter the password `amethystora` when prompted to enroll the key.

If this step is not completed during the initial setup, you can manually enroll the key by running the following command in the terminal:

```bash
ame security secure-boot
```

The public key can be found in the akmods repository [here](https://github.com/ublue-os/akmods/raw/main/certs/public_key.der).
If you'd like to enroll this key prior to installation or rebase, download the key and run the following:

```bash
sudo mokutil --timeout -1
sudo mokutil --import public_key.der
```

## Branding

The logo, the Amethystora wallpapers, every theme's drawn wallpapers, boot splash and fastfetch logo, with the glint that crosses it in `fetch`, are generated from [branding/generate.mjs](branding/generate.mjs). To change the colours or shapes, edit that script and re-run it (needs Node.js and Edge or Chrome):

```bash
node branding/generate.mjs
```

[build_files/base/06-branding.sh](build_files/base/06-branding.sh) puts the artwork where the boot menu, the boot splash and GDM look for it.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Issues and pull requests are welcome at [arsslensoft/amethystora](https://github.com/arsslensoft/amethystora).

## Acknowledgements

Amethystora is based on [Universal Blue](https://universal-blue.org/) and keeps building on its shared infrastructure. Thanks to the Universal Blue contributors for the work this project stands on.

The theme switcher follows the design of [Omakub](https://omakub.org) and its fork [Omabuntu](https://omabuntu.omakasui.org/), and most of the colour palettes are ported from them (MIT). The boot menu follows the theme layout of [grub2-themes](https://github.com/vinceliuice/grub2-themes) by Vince Liuice (GPL-3.0). The icon packs are drawn for Amethystora. The themes' wallpapers are drawn for Amethystora and listed in [CREDITS.md](system_files/shared/usr/share/backgrounds/amethystora/CREDITS.md).

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) and [NOTICE](NOTICE) files for details. A few of its files are other people's work under licences of their own (MIT, GPL); NOTICE names each one.

### Third-Party Components

Amethystora incorporates and builds upon several open source projects:
- **Universal Blue**: Base images, the desktop layer, kernel modules and build tooling
- **Fedora Linux**: Base operating system
- **GNOME Desktop Environment**: Desktop interface
- **Various CNCF Projects**: Cloud-native tooling and containers

All incorporated components keep their respective licenses and attributions.

Fedora and the Fedora logo are trademarks of Red Hat, Inc. Amethystora is built from Fedora's packages and is not provided, supported or endorsed by the Fedora Project or by Red Hat. Fedora itself is at [fedoraproject.org](https://fedoraproject.org).
