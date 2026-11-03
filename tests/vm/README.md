# The VM lab

A virtual machine for trying an Amethystora image as it boots: UEFI with Secure Boot and Microsoft's keys, a
TPM 2.0, GNOME on a 1280×800 screen. It builds the image from the last commit, boots it and runs the test
suites in it (`run.sh`), and an agent drives it by hand through an MCP server (`server.mjs`).

Nothing of the lab is in the image. Commands reach the guest through `agent.py`, which QEMU hands to systemd as
a credential at boot (SMBIOS type 11) and which runs from `/run` until the VM shuts down. It answers on a
virtio-serial port no other machine can reach. The screen, keys and mouse go through QEMU's own monitor. The
security watcher may say that a unit it does not know ran: that is the lab's helper.

## Setting it up

The lab is a privileged Fedora container, `amevm`, with QEMU, KVM, OVMF, swtpm, and what `just build`
needs. It needs Docker or Podman on Linux, or Docker Desktop on Windows (nested virtualization in WSL 2 gives
it `/dev/kvm`, once `setup.sh` has loaded the module).

```bash
bash tests/vm/setup.sh
```

`setup.sh` builds the lab's image and starts the container again. Disks and VMs stay in the volume
`amevm-vm`, pulled and built images in `amevm-storage`; VMs that were running are stopped. Run it again after
changing the `Containerfile`; a test run brings the other files here up to date itself.

To give the MCP server to Claude Code, for this checkout only:

```bash
claude mcp add amethystora-vm --scope local -- docker exec -i amevm node /opt/amethystora-vm/server.mjs
```

The tools appear in the next session.

## Test runs

```bash
bash tests/vm/run.sh                # build HEAD, install it onto a disk, boot it, run the suites
bash tests/vm/run.sh build|disk|test   # one stage, from what the last run left
bash tests/vm/run.sh test --keep    # and leave the VM ("test") running afterwards
```

`run.sh` sends the last commit into the lab as a git bundle (whatever the checkout's line endings),
and `run.mjs` builds it with `just build amethystora latest main` (a throwaway module signing key, as every
build without the release key), installs it with `make-disk.sh`, and runs each suite in `suites/` as root
in the VM, in order. With `VMLAB_NAME=NAME`, a run has its own source tree, image tag
(`localhost/amethystora-run:NAME`), disks and VMs, and its files go to `/vm/runs/last-NAME`, so that two
sessions can share the lab; the build itself tags `localhost/amethystora:latest` first, so two builds at once
are agreed on beforehand. The disk's firmware has the image's own certificates in shim's MOK list, as on a
machine whose owner enrolled them. Everything a run did is in the lab's `/vm/runs/last`: one log per stage
and per suite, a screenshot after each suite, the guest's journal, `results.json` and `summary.md`, which
`run.sh` prints last. It exits 0 only when everything passed. The `run-tests` skill in `.claude/skills`
is how an agent runs it and fixes what fails.

### Several VMs at once

```bash
bash tests/vm/run.sh test --parallel 3              # the suites spread over three VMs
VMLAB_NAME=x bash tests/vm/run.sh plan --parallel 3 # the lanes it would run, and nothing else
```

`--parallel N` first makes a ready state: VM `test` starts on the disk, runs the suites `shards.json` calls `prep`
(the boot, the first-run setup, the sign-in), waits for the first boot's Flatpaks and powers itself off. VMs
`test-p1` to `test-pN` then start from it, each an overlay on its overlay (`vm_start`'s `from`): nothing is copied,
and QEMU refuses to start `test` again while one of them runs. The other suites are spread over the N lanes by their
minutes in `shards.json`, longest first onto the lane that would end soonest; a lane runs its suites in name order,
`last` ones last. The installer's suites run on `test-installer-pK`, a fresh overlay of that disk, while the ready
state is being made; the screens suites run together on `test-screens`, from the ready state. A suite `shards.json`
does not name counts `default_minutes`: give it its minutes from a run's `results.json` once it has one. The VMs
are deleted at the end (not with `--keep`), the ready state is kept until the next run, and `summary.md` says which
VM ran each suite and how long each lane took. `--memory MB` gives every VM but the screens one that much memory;
`--reuse` starts from the ready state the last run made on the same disk instead of making it again (`run.sh test
--parallel 3 --reuse --suites 11,12`, to run a few suites again after a fix).

Each suite starts on a machine where only the prep suites ran before it, not every suite before it in name order:
a suite that needs another's leftovers says so and does the work itself.

The lab shares one Linux VM with every other container (WSL 2's, 13.3 GB here), and when a build and a VM ran
beside other containers it ran out of memory and Docker stopped answering (2026-10-29). So `vm_start` refuses while
an image is being built in the lab, and when the VM's memory is not free: the kernel's `MemAvailable`, less what
the running VMs have yet to take (their `-m` less their resident memory), less 1.5 GB. A run waits up to 15
minutes for the memory, one start at a time, and `--parallel N` refuses at once, saying how many VMs fit, when N do
not.

### Suites

A suite is `suites/NN-name.sh`, run as root in the VM beside `lib.sh`, which it sources. It says one line per
check, as TAP does (`ok - ...`, `not ok - ...` with indented lines saying why, `skip - ...`), uses `check`,
`wait_for`, `as_user` and `in_session` from `lib.sh`, and ends with `finish`. Exit status 100
(`reboot_and_continue`) restarts the VM and runs the suite again with `SUITE_PHASE` one higher, sixteen phases at
most; a restart empties `/run`, so what a suite keeps between phases goes in `/var/lib/vmlab-tests`. A line
starting with `#` is a measurement, which the report repeats. A phase that calls `next_boot JSON` first gets a
power cycle instead, the machine started again with a spare disk unplugged (`{"absent": [2]}`), with sectors
that fail every read (`{"faulty": {"3": [sector, ...]}}`, through QEMU's blkdebug), or with the disk's
passphrase typed at its prompt rather than handed over (`{"type_passphrase": true}`; the next phase finds how
many times it was asked in `VMLAB_PROMPTS`), with the boot menu's last entry chosen as it shows on the
serial console (`{"menu_last": true}`, End and Enter), with its default entry opened for editing first
(`{"menu_edit": true}`, e; the next phase finds in `VMLAB_MENU_EDIT` whether GRUB `asked` for a user or opened
its `editor`), without its own disk (`{"system_absent": true}`), or with a
credential left out (`{"credentials": {"cryptsetup.passphrase": null}}`, for the TPM alone). A later plain
restart keeps the machine as that start left it. `{"itself": true}` is no power cycle: the phase has left
something to restart the machine once it has ended, as a person would, and `run.mjs` only waits for it, a soft
reboot too (it keeps the boot id, so systemd's `SoftRebootsCount` is compared as well); with
`{"powers_off": true}` it waits for the machine to power itself off, then starts it. Exit status 101
(`continue_phase`) runs the next phase in the same boot, for what outlasts a phase's 30 minutes; a phase that calls
`usb_next JSON` first has USB devices plugged out and in before it, as a person would (`{"unplug": ["stick"],
"plug": ["stick", "keyboard"]}`: two USB sticks with serials of their own and a plain keyboard, `USB_DEVICES` in
`run.mjs`).

A suite whose second line is `# vmlab-vm: installer` runs on a second machine, installed as the installer
lays a disk out: an ESP, an ext4 `/boot`, and btrfs (subvolumes `root` and `home`) on LUKS2, with the fstab and
crypttab lines Anaconda writes and the serial console in its kernel arguments (`make-disk.sh --installer`). Its
passphrase (`VMLAB_PASSPHRASE`) and a TPM PIN for suites to enrol (`VMLAB_TPM_PIN`) reach systemd-cryptsetup as
credentials, so nothing has to be typed at boot. It has four spare disks, two of 8 GiB and two of 48 GiB,
larger than its own 40 GiB.

A suite whose second line is `# vmlab-vm: screens` runs on the first machine's disk with four screens of 1280×800
side by side (`vm_start`'s `screens`, each after the first a `secondary-vga` of its own, which GNOME lays out left to
right); `keys.py`'s fractions are then of all four, and `vm_screenshot` and `vm_mouse` take a `screen`. It signs its
account in itself (`10-account.sh`).

A machine's suites run in the order of their names, except one that says `# vmlab-order: last` in its first three
lines: it leaves the machine of no use to the suites after it, and runs after all of them. `90-control-switch.sh`
does, since it switches to the developer image and ends with Start over, which leaves no test account.

What asks a person something (gum, a typed `ERASE`, a check for a terminal) runs through `in_terminal`, which
gives the command a terminal of its own (`term.py`), waits for each question and types its answer. The test VM
has five blank SATA disks besides its own, serials `SPARE-1` to `SPARE-5` (four of 8 GiB, then 10 GiB), new for
every run, and a sixth of 24 GiB, more than the system's data, which stays unplugged until a suite asks for it
with `next_boot '{"absent": []}'`. Its home folders are not a btrfs subvolume of their own, as the installer
would make them: a suite that needs one (the hourly snapshots do) calls `make_home_subvolume` unless `home_subvolume` says it is one
already, then restarts.

An app's window is used as a person would use it, through the accessibility tree every GTK window has:
`atspi.py`, run as the account on its session's bus, finds, clicks and types into what the window shows and
reads what it says (`70-backups.sh` shows how).

| Suite | What it checks |
| --- | --- |
| `00-boot.sh` | every unit started, Secure Boot and lockdown on, a TPM, the image's own module loads, nothing crashed |
| `05-first-run.sh` | GNOME's first-run setup starts on a machine with no account, and no per-account setup runs in it |
| `10-account.sh` | the test account `tester` signs in on Wayland, and its first login's setup runs |
| `20-apps.sh` | each Amethystora app opens and stays open |
| `22-screen-text.sh` | copying text out of the screen: Gradia and its extension are there, Gradia's language download is where it fetches it, and with a language file in its data folder `Print`, an area dragged over a window showing a known sentence and `Ctrl+E` (Extract Text) put the sentence on the clipboard and leave the capture in the screenshot folder |
| `25-every-app.sh` | every other app in the app grid opens as the grid starts it and stays open, none crashes or is refused what its manifest asks for (Mission Center's gatherer runs), and neither opening them all nor a restart and a login makes the security watcher say anything |
| `26-logo-menu.sh` | the logo menu at the left end of the top bar: on, leftmost, its eight entries in order each opening its app's window, a middle click showing and hiding the overview, Mission Center uninstalled hiding its entry (and nothing installing it), back after the screen lock, and turned on at the next login of an account whose list of extensions still names the Logo Menu |
| `27-amethystora-ui.sh` | Amethystora UI as a session has it: the extension active with no module failing or throwing, none of the extensions it replaced installed, the tiling one switch (classic gives GNOME's Super+N back, tiling takes it again, its state file says so), the icon packs (Glass on the dark theme, Duotone on the light one, Neon by choice, candy-icons refused, GTK finds every launcher's icon in each), the window lights in the account's GTK stylesheets, and the dock at the bottom of the shell's accessibility tree; two Files windows are left open for the run's screenshot of the strip |
| `29-screens.sh` | on four screens: the six fixed workspaces (also for an account that set dynamic ones), a workspace on each screen, a Files window taken from screen to screen with Super+Shift+Ctrl+arrows and dragged across a screen to the next, the search box open while the pointer crosses a screen with a window, glass on the windows of a screen without the keyboard, the top bar solid under a window of its screen's workspace, nothing thrown |
| `30-detection.sh` | the thorough profile, after a restart, finds a secret read, the decoy read and a program run from Downloads; Falco's memory |
| `40-setup.sh` | `ame setup` save, diff and apply against real GNOME settings, Flathub and Homebrew; the setup kept with a backup and read back by `ame restore-setup`; `ame system trust-image`, a switch to an unsigned image refused, `amethystora-image-check` |
| `44-what-leaves.sh` | what the machine sends by itself: `ame security sends` and the report's group, Fedora's count, NetworkManager's connectivity check (also after GNOME's own switch), the firmware catalogue and the account's location each way as they work, then ten idle minutes with resolved's monitor: every name looked up is one the list names (`WHAT_LEAVES_IDLE` seconds) |
| `45-network.sh` | on networks made in the VM (dnsmasq in a network namespace, a web server answering with a login page, a VPN's link): `ame security mac` (an address and a DHCPv6 identity per network, no name told), `ame security dns` (DNS-over-TLS on the wire, the network's own names, the connectivity check and the VPN's on their own servers), a network that wants a login first (the offer and the pause), Firefox through the system's resolver and with its own DNS-over-HTTPS |
| `46-trust.sh` | on the installer-layout machine: the kickstarts' signed `bootc switch` with no network and `amethystora-signed-updates`; `ame security lockdown` off and on across restarts (and without Secure Boot in a VM started so); `ame system dx-group`, the report's and Control's words |
| `47-app-integrity.sh` | app integrity on: a small Flathub app for the machine and one for the test account checked clean, and what each check costs (the first, one with nothing new, one after an update); a deployed file replaced and put back by `ame security app-integrity repair`, an object written in place, a stray launcher, the account's own app changed (said to it, put back without a password), the system's own object and initramfs changed and put back; the app permissions' `code` and `installations`; a library preloaded from a home, and a session's environment |
| `48-injection.sh` | `proc_mem.force_override=ptrace` on the command line, and a program's write over its own code refused; with INJECTION on, from the next boot: an attach to a process the account did not start refused and said, its own child attached to, read, written into and its `/proc/PID/mem` opened, each said, and the same inside one Flatpak recorded only |
| `50-raid.sh` | pools of disks on the spare disks: parity, two copies encrypted and combined made, restarted, scrubbed (a scrub cut short is resumed), a disk taken away while mounted and one missing at boot, opened degraded, replaced, added to and forgotten; the recovery key, Control's words, no TPM unlock for a pool; a disk pulled out of an encrypted pool, one with bad sectors (the notification's Open Control clicked) replaced while it reads, a copy damaged behind btrfs's back repaired by a scrub, a suspend during a scrub (the kernel's own test of it, `pm_test`: the VM has no S3, and nothing wakes it from s2idle), a disk unplugged at boot and plugged back, its copies put back |
| `55-raid-system.sh` | on the installer-layout machine: the system mirrored onto a new disk (2b), the passphrase asked once, the restart from the new disk finishing it, the TPM on both disks and not a pool's, an update on both halves of `/boot` and both ESPs, each disk dead in turn (Start with a disk missing), one back with its copies put back, the other replaced |
| `60-vanilla.sh` | `amepkg`: an Alpine container with a launcher exported and taken back, the daily upgrade, Apx files imported, a `.deb` from a file scanned and installed; the watcher's fake `sudo` and stray launcher; the NVIDIA offer on stand-in hardware; "You're on the previous version" and `bootc rollback` |
| `70-backups.sh` | the home folders made a btrfs subvolume (a restart), the hourly snapshots and a backup to an append-only rest-server; Backups' helper and window on both: points, folders, versions, putting back (labels, reflinks, refusals), Look, the whole folder, Open With, a stale and a failed backup |
| `74-vault.sh` | Amethystora Vault as a person goes through it, with real polkit, SELinux enforcing and the real service: the sockets and nothing running or kept before setup, the cold start, setup in a terminal with the recovery key typed back, the folder, `/proc`, the network, another account, a Flatpak and an account over SSH all kept out, the sealed purposes Round 7 builds on, each item's approval and the sentence polkit shows (as a text agent prints it and in GNOME Shell's dialog, read through AT-SPI, with the lab's screenshot checked), the ways it opens (the account's password, the TPM, no prompt, a fingerprint on libfprint's virtual reader), what closes it (the screen, a suspend, an idle minute, the service killed, no core file), what idles (no process, an open vault's wakeups and memory), an authenticator code against `oathtool`, the SSH agent with `ssh` to a real sshd and in a new login, a protected file run through `ame vault run` (and what the account's other programs can reach meanwhile), the machine's root, a certificate and a real PDF signed through the PKCS#11 module and read by `pdfsig` as trusted and as unknown, a hidden vault (in no list, journal or report, and a password that takes as long as a wrong one), the sealed backup, forgetting and restoring with the recovery key, Vaultwarden in a container reached by the network service alone, no denial; then after a restart. Not run: a security key, the camera and a picture's QR, a second account with a seat, Bitwarden's own server |
| `75-deja-dup.sh` | Déjà Dup from Flathub: where its schedule is kept, what `flatpak preinstall` uninstalls when its list stops naming an app, and the one notice at the next login |
| `83-logins.sh` | Parental Controls' allowed hours, a child's login typed at the text console and made over SSH; one-time codes for SSH set up and switched in a terminal, a code used twice, a scratch code, a key, fail2ban banning wrong codes from another address, what the watcher accepts; an Active Directory domain (a Samba domain controller in a container) joined with SSSD and with winbind, logged in to and left, the image's PAM setup kept each time |
| `85-raid-mirror.sh` | the system spread over two disks as the installer leaves it (SPARE-6), mirrored (2a), started with the second disk unplugged from Start with a disk missing, the disk back and its copies put back, then on one disk again |
| `86-locked-session.sh` | `ame security usb locked` turned on in the running session: a stick and a keyboard plugged in while the screen is locked (the stick blocked, the keyboard let in, GNOME's notifications), the stick still blocked once unlocked until plugged in again, the login screen of another session counting as unlocked, `on` afterwards (no rule of GNOME's left, a new stick blocked while unlocked) and off; the private folder made in a terminal with its recovery key, closed by the lock with programs holding files in it (named in a notification), its thumbnails and recent files gone, opened from the launcher with a wrong and the right password, Close Now, its watcher stopped or killed, a suspend without the lock, a new password with the old one and with the recovery key, a backup holding only the encrypted copy, logging out, removing |
| `89-soft-reboot.sh` | on the installer-layout machine: the initramfs without the build's version or an out-of-tree driver; updates made from the running system (a hardlink checkout of its commit) and staged, which `amethystora-soft-reboot` calls quick (a file added, a loaded driver compressed anew) or not and why (a kernel argument, the driver changed, BootNext set); its status, prepare and reset as the account, an app inhibiting logout; System Updates' Restart now asking first and ending in a soft reboot (same boot id, the update running, `/etc` carried over, the account back), then a full restart into the same update |
| `91-clipboard.sh` | Amethystora Clipboard, copied to as apps copy (`wl-copy`, and `clipcopy.py`'s own GTK window) and read back from its menu and from disk with the keyring's key: with no keyring at login the menu says nothing is saved; Clipboard Indicator's plain history and a setting taken over; text and pictures sealed and nowhere in the home in the clear; a copy marked secret and one from KeePassXC's window not recorded; the history outlasting the screen lock and a hundred copies in a burst; after a restart (signed in without the password, as by a fingerprint) the keyring locked, earlier items held back and a new copy kept in memory only, then the menu's row and GNOME's prompt unlocking it and merging the two; `ame desktop clipboard` restart and always across restarts |
| `92-hardening.sh` | on the installer-layout machine: every prevention switch of `amethystora-hardening` once each way, at once and after a restart, as it works: the firewall's zones, the away zone and a network named home from another network namespace, account lockout through PAM, Bluetooth privacy through BlueZ on a virtual controller (`/dev/vhci`), nothing started from the temporary folders while bootc still updates, no X11 at the next login, the boot menu asking for its password to edit an entry (and its editor without one), kernel arguments switched on a machine with a layered package, the audit rules locked in the sensor's place, the machine powering itself off after 18 hours locked; the watcher seeing no drift and saying nothing louder than a note |
| `93-privacy.sh` | The trash emptied after 30 days as the image ships it, `ame desktop privacy trash 7` taken by the running session at once, and GNOME's hourly purge waited for: files deleted 40 and 10 days ago gone, today's kept, a snapshot and a restic repository untouched; off, and Reset settings putting 30 days back |
| `94-vpn.sh` | the VPNs people are given at work: NetworkManager keeps a connection for each of the fortisslvpn, l2tp, libreswan, sstp, openconnect, openvpn and vpnc plugins and imports an `.ovpn` file; L2TP's `l2tp_netlink` and `l2tp_ppp` load by alias (the way its daemon gets them, which Fedora's blacklist would stop) while `l2tp_eth` stays blocked; IPsec's modules load unless `kernel strict` is on; `ipsec.service` is off until started, starts and stops; SELinux denies none of the VPN programs |
| `95-phone-link.sh` | The phone link on a machine with no phone: GSConnect and its Files extension gone, the program, the Phone app and the user unit in the image, the settings only the name and the two clipboard switches (both off), the home zone alone open to phones; off as shipped (not enabled, not running, no bus name, nothing on port 1716), `ame desktop phone on` starting it (bus name, listener the watcher allows, no phones, its folder 0700) and `off` taking all of it away |
| `97-terminal.sh` | kitty as the terminal: `xdg-terminal-exec` opens it and it stays open, the image's `kitty.conf` is what it reads, a theme switch rewrites its colours and an open window takes them without closing, and the prompt is the powerline one (account, folder, branch, an arrow after each, the command on a line of its own) in the theme's colours |

A new feature gets its check here with it, and a bug a run found gets the check that would have caught it.

## Using it by hand

1. `disk_build` installs an image onto a disk with `bootc install to-disk` (btrfs, as the installer ISO
   makes it), about 10 minutes. `disk_list` says when it is ready.
2. `vm_start` starts a VM on an overlay of that disk, with spare disks (`spares_gb`), some of them `absent` or
   `faulty`, `credentials` for systemd, and up to four `screens`; `vm_wait` returns once the boot is over.
3. `vm_exec` runs a command as root, as an account, or in that account's graphical session (`session`).
   `vm_write` and `vm_read` move files.
4. `vm_screenshot`, `vm_keys` and `vm_mouse` see and use the desktop (one `screen` at a time; `vm_mouse` also
   presses and releases, for a drag); `vm_serial` is the serial console.
5. `vm_snapshot` keeps checkpoints of a stopped VM; `vm_stop` and `vm_delete` end it.

A new disk has no account and starts GNOME's first-run setup; `suites/10-account.sh` shows how to sign a test
account in. The VM has 4 GB of memory and 4 processors unless `vm_start` says otherwise. Under nested
virtualization GNOME draws in software and is slow: the screen can show a click a few seconds late.
