# How Amethystora works

Written for AI agents. Amethystora is an image-based desktop built on Fedora with GNOME: the whole system
is a signed container image, installed and updated whole by bootc and ostree. Read this before deciding
where a fix goes.

## The image and its three folders

- `/usr` is the image: read-only, replaced whole on every update. Never try to edit it; read it to learn
  the defaults. `/usr/share/amethystora` holds Amethystora's own themes, manual, skills and this knowledge.
- `/etc` is writable and kept across updates. Each update does a three-way merge: a file changed on the
  machine keeps the change; a file nobody changed follows the new image. The image's own copy of every
  file in `/etc` is in `/usr/etc`, so `diff /usr/etc/FILE /etc/FILE` shows what the machine changed. The
  security watcher reports such drift under **Image settings**.
- `/var` is the machine's data, never touched by an update: `/var/home` (the homes; `/home` points there),
  `/var/lib/flatpak`, `/var/log`, container storage. A file put in `/var` stays.
- Everything else (`/opt`, `/usr/local`) is a link into `/var`.

## Deployments and rolling back

- An update downloads a new image and stages it as a new deployment; it becomes the system at the next
  restart. The previous one is kept, and the boot menu's second entry starts it.
- `rpm-ostree status` (or `/usr/libexec/amethystora-deployments`, which prints the same shape on every
  image) lists them: booted, staged, rollback. `state overview` and `state updates` summarise it.
- Started from an older deployment than the default, the machine runs it until the next restart;
  `sudo bootc rollback` keeps it as the default. The next update brings the newest back.
- Sealed images (`/usr/share/amethystora/sealed` exists) have no rpm-ostree: `sudo bootc status` shows the
  deployments, and their kernel arguments are inside the signed kernel image, which nothing on the machine
  can change.

## Kernel arguments

The image's kernel arguments are in `/usr/lib/bootc/kargs.d/`, and bootc applies them when it updates. A
machine with layered packages updates through rpm-ostree instead, which ignores `kargs.d`: a kernel
argument the image added later does not reach it. `cat /proc/cmdline` shows what the running kernel got;
`rpm-ostree kargs` shows and changes the arguments of the next start (root, restart).

## How updates run

- uupd updates the image, the Flatpaks, Homebrew and the firmware's metadata together: `uupd.service` on
  `uupd.timer` (automatic), and `uupd-manual.service`, which the Updates app starts without a password.
  `ame update` does the same work itself in a terminal (rpm-ostree or bootc, then Flatpak and Homebrew), and
  refuses while `uupd.service` runs. `journalctl -u uupd.service -u uupd-manual.service` is uupd's log.
- Only images signed with Amethystora's key are installed (`/etc/containers/policy.json`;
  `amethystora-signed-updates.service` keeps the machine on signed updates at every start).
- The containers of amethystora-pkg upgrade daily for each account: `amethystora-pkg-upgrade.timer` (user);
  the Updates app shows them as a step of their own.
- Firmware comes from LVFS through fwupd: `fwupdmgr get-updates`, `fwupdmgr update` (may restart). The update
  alert says once for each release that one waits, and the security report's `firmware` check names them.
- Drivers: `amethystora-driver-check.timer` (root, a few minutes after each start) saves what keeps a device
  from working (a missing driver, missing firmware, a refused module, NVIDIA's driver not loaded, a Broadcom
  Wi-Fi card no driver drives, the Xbox wireless adapter without its firmware, a DisplayLink dock whose driver
  did not load) for `ame system drivers`, and says each once. The kmods beside the kernel (v4l2loopback, xone, xpadneo, evdi, NVIDIA's) are signed with
  keys `ame security secure-boot` enrolls.
- `ame system rebase IMAGE STREAM` switches image (with or without NVIDIA's driver: `-nvidia-open` for
  Turing and newer, `-nvidia-lts` for Maxwell to Volta) or stream (stable, latest, beta); only signed images,
  and to an NVIDIA image with Secure Boot on only once the keys are enrolled.

## Where software goes

In this order: Flatpak (apps with windows, `flatpak install flathub APP`), Homebrew (command-line tools,
`brew install`), amethystora-pkg containers (another distribution's packages, `amepkg`), and last
`rpm-ostree install`, which layers a package into the image: it slows every update, needs a restart and
stops kargs.d from applying. There is no `dnf install` on the host. Sealed images take no layered packages.

- Flatpaks run sandboxed; the image takes X11, the input devices and the Flatpak service from every app
  (`/etc/flatpak/overrides/global`); `flatpak override --user` gives one app one thing back.
- Containers are distrobox containers made from pinned templates; they share the home, the display and the
  session bus, so they are not a sandbox.
- A `.deb`, `.rpm` or `.pkg.tar.zst` file: `amepkg install FILE` scans it and puts it in the container it
  fits, offering Flathub first; it asks in a terminal, so the user runs it.
- A Windows program: `/usr/libexec/amethystora-windows-program FILE` (what Files opens one with) scans it, says
  who signed it and offers the same app for Linux from `/usr/share/amethystora/windows-programs.json` before
  Bottles, from Flathub, granted X11 for the account alone; it asks in a terminal, so the user runs it.

## User and system scope

- Each account's own: `~/.config/amethystora` (its switches: `agentic`, `agent-tools`, `layout`, the theme
  in `current`), `~/.local/share/flatpak`, `~/.var/app/APP` (an app's data), `~/.local/state/amethystora`,
  user units (`systemctl --user`), dconf (`gsettings`).
- The machine's: `/etc/amethystora/security.conf` (detection switches, `ame security settings`),
  `/etc/amethystora/agent.conf` (the agent's lock and root mode), `/etc/amethystora/hardening-off` and
  `hardening-on` (what `ame security` switched), system units, `/var/lib/flatpak`.

## Where state lives

| What | Read it with |
| --- | --- |
| Amethystora's settings outside security | `state settings`, `/usr/libexec/amethystora-control-list --json` |
| The security report and its switches | `state security`, `state security-settings`, `ame security status` |
| What each Flatpak can reach | `state app-permissions`, `ame security apps` |
| Deployments and updates | `state updates`, `rpm-ostree status` |
| Backups and snapshots | `state backups`, `/usr/libexec/amethystora-restore status --json` |
| Pools of disks | `state disks`, `/usr/libexec/amethystora-raid status --json` |
| Failed services, crashes | `state units`, `state crashes`, `systemctl --failed`, `coredumpctl list` |
| What is wrong overall | `state health`, `amethystora-doctor` |
| The journal | `logs`, `journalctl` |
