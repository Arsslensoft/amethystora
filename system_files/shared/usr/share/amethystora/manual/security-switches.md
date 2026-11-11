# Switches and settings

Every security feature here is a switch you can turn off or on, in the Security app or with an `ame security`
command. Off costs nothing.

## Switches

Every security feature is a switch, so that you can take back any of it if it makes this machine slower
or gets in your way. Off costs nothing: nothing runs, nothing is downloaded and nothing is said. The
audit log's rules are locked until the next restart, so a part of it you turn off stops being read at
once and stops being recorded from the next restart.

### Everything at once

`ame security profile off` turns off everything that can be: the watcher, the virus scanner, the settings
audit, real-time watching, network protection and the audit log. `ame security profile default` puts every
switch back as the image ships it, which also turns off the ones that send something away (site checking,
asking the agent about events, hash reputation). The Settings page has both as buttons.

### Profiles for new threats

Each profile sets [what looks for new threats](new-threats.md), sets every
other switch to its default, and never turns on what sends something to somebody else. The Settings page
offers the three as one choice. Every switch can still be changed alone afterwards; the page then says the
switches are changed from the profile.

| Profile | What it does |
| --- | --- |
| `ame security profile light` | Records the cheapest; says ransomware, agents' settings and apps that gain permissions; records what reaches into another program's memory |
| `ame security profile balanced` | All of those said; records secrets read, programs run from the downloads and programs loaded into the kernel; puts the chains of steps together; fetches the command-server lists daily |
| `ame security profile thorough` | All of it, the decoy key and the YARA rules included; records what the behaviour sensor, odd DNS names and the hunting rules find. Costs memory, CPU and downloads, and on a developer's machine some noise |

### From a text console

Each service reads its switch as it starts, so a line edited by hand in `/etc/amethystora/security.conf`,
such as `VIRUS_SCAN=off`, takes effect at the next restart even when the desktop does not start.

### Off is not a warning

A feature you switched off is shown in the report as turned off, with the command that turns it back on.
Turned off some other way, such as a service disabled with `systemctl`, it is still a warning, because
that is what it would look like if something else did it.

A change made through a switch is recorded by the watcher and not said. A change made any other way is
said.

### A warning you cannot fix

A warning you cannot fix on this machine, such as Secure Boot on a computer without it, can be accepted
with **Accept** in the Security app, or `ame security allow report`. It stays in the report as
information, out of the count, and is never said, until it turns worse. **Show it again**, or
`ame security allow report <check> remove`, shows it again.

### Something you know is fine

An event on the **Events** page that names a file or a program has **Allow…**: what matches it is
recorded from then on, and no longer said. Allow only what you know: whatever matches, malware included,
goes unannounced from then on.

```bash
ame security allow persistence '~/.local/share/gnome-shell/extensions/*'   # a path or glob; ~/ is every home
ame security allow persistence                                             # what is allowed
ame security allow FEATURE ENTRY remove                                    # take an entry back
```

The image allows a few of its own the same way, such as Fedora's sssd and tuned rewriting their settings
at every start and fwupd's passim listening for nearby machines: `ame security allow hardening` and
`ame security allow listeners` list them. Most features take a path or a program, or a glob of them; a few
take two parts, in quotes:

| Feature | An entry |
| --- | --- |
| `identity`, `privilege`, `persistence`, `hardening`, `modules`, `input-capture`, `user-programs`, `dev-hooks`, `agent-config`, `exec-watch`, `agent-clis`, `decoy`, `snapshot-watch` | A path, or the program that wrote there |
| `processes` | The same, or a variable and the path it names: `'LD_LIBRARY_PATH ~/opt/toolchain/lib'` |
| `injection` | A program, alone or with the program it reached: `'/usr/bin/x /usr/bin/y'` |
| `selinux-denials` | Source and target types, class and permissions: `'xdm_t user_home_t file read,open'` |
| `credentials` | A program and the secrets it may read: `'/usr/bin/restic *'` |
| `behaviour` | A program and the behaviour sensor's rule it set off |
| `dns-names` | A name: `'*.example.net'` |
| `bpf-programs` | A BPF program's name and what loaded it, or `unknown` |
| `app-permissions` | An app's id and a way out of its sandbox: `'com.valvesoftware.Steam x11'` |
| `listeners` | A program and its port, or `ephemeral`: `'syncthing *'` |

### What the image enforces

What the image enforces from the start has switches of its own, each on until you turn it off, each in the
Settings page and as a command that says what it changes before it does. `ame security lockdown` and the
[stricter settings](#stricter-settings) are switches already. Something with root can change any of this,
but not quietly: each change is recorded by the audit log, until you turn that off too.

| Command | What it switches | Turned off |
| --- | --- | --- |
| `ame security kernel` | Kernel addresses and messages hidden, programs kept from each other's memory, no unprivileged BPF, a panic after a hundred oopses, no kexec, a hardened BPF compiler, no unprivileged userfaultfd, files in shared folders such as `/tmp` protected from other accounts, and no ICMP redirects or source routing; `standard` is this level again after [`strict`](#stricter-settings) | From the next restart. `gdb -p`, `strace -p`, `perf` and `dmesg` work without `sudo` |
| `ame security blocked-modules` | Old network protocols and filesystems, test and fault-injection drivers, CAN, GNSS, the floppy and parallel ports kept from loading | At once. Disks from old Macs, SCTP, CAN buses, amateur radio |
| `ame security firewire` | FireWire kept from loading, since a FireWire device reads and writes memory by itself | At once. FireWire sound cards, DV cameras and disks ([Hardware](devices.md#sound-cards)) |
| `ame security kernel-args` | `init_on_alloc=1`, `slab_nomerge`, `page_alloc.shuffle=1`, `randomize_kstack_offset=on`, `vsyscall=none`, `proc_mem.force_override=ptrace` (only a program's own debugger writes over its code): `off` or `on` alone switches all of them, `ame security kernel-args vsyscall=none off` one | From the next restart. Not on a sealed image. Older releases of the `rr` debugger need the last one off |
| `ame security firewall` | The Amethystora firewall zone | At once. Fedora Workstation's zone lets in every port from 1025 up; opening one port is the narrower answer |
| `ame security failed-logins` | Blocking an address after failed SSH logins | At once |
| `ame security lockout` | Locking an account after ten wrong passwords | At once |
| `ame security app-sandbox` | Taking X11, the input devices and the Flatpak service from every app | From each app's next start; granting one app X11 in Flatseal is the narrower answer |
| `ame security virtual-input` | `/dev/uinput`, which makes a virtual keyboard that types into any window, kept from apps | At once, and back on from the next restart. Steam Input and OpenTabletDriver need it off, and their setup turns it off |
| `ame security ssh-settings` | No root login, three tries and no X11 forwarding for the SSH server, and no SSH over vsock from the host of a virtual machine | At once |
| `ame security signed-updates` | Switching this machine back to signature-checked updates at every start | The machine stays on what it follows now; turned on again, from the next start. For an image of your own, `ame system trust-image` is the answer |
| `ame security ipv6-privacy` | A temporary IPv6 address that changes, on every connection | From each network's next connection |
| `ame security bluetooth-privacy` | Bluetooth's random address, and visible and pairable for 30 seconds at a time | At once; Bluetooth restarts |
| `ame security plugin-approval` | An administrator approves installing a [plugin](plugins.md), in the system's own dialog | At once. Off, any program running as an administrator, an AI agent's shell included, can install a plugin and give it what it asks for; each time is recorded and said afterwards. Widening, unblocking and rolling back still ask |

## Stricter settings

Each of these takes something away that a desktop may need, so each is off until you turn it on, in the
Settings page or with its command, which says what it costs first. The report lists the ones that are
on, in case one is why something stopped working.

| Command | What it does | What it costs |
| --- | --- | --- |
| `ame security kernel strict` | No io_uring, debuggers attach only as root, more address randomisation, no IPsec; `ame security kernel standard` goes back | QEMU, Samba and some Node programs lose io_uring; `gdb -p` needs `sudo`; sanitizers; libreswan and strongSwan |
| `ame security kernel-args <argument> on` | `init_on_free=1`, `iommu.strict=1` (the IOMMU on, and strict), `ia32_emulation=0`, each on its own | Slower; slower disks and network; no Steam or 32-bit Wine |
| `ame security lockdown strict` | Root cannot read the running kernel either, with Secure Boot on too; `ame security lockdown on` goes back, from the next restart | `bpftrace`, `perf` with kernel data, eBPF security tools |
| `ame security time` | The clock set only from time servers that prove who they are (NTS), three agreeing, never one a network suggests | Networks that block port 4460 leave the clock unsynced |
| `ame security firewall-away` | Networks you have not named home answer nothing; it asks which are home first, and `ame security home-networks` names more. Not while the firewall zone is off | Printers and shares only at home, as phone pairing always is |
| `ame security network-daemons` | Avahi, ModemManager and passim (fwupd's sharing of firmware updates with machines nearby) stopped | Printers by address only; no mobile broadband; each machine downloads its own firmware updates |
| `ame security block-bluetooth` | Bluetooth cannot load | Headphones, mice and keyboards that use it |
| `ame security block-webcam` | The USB webcam driver cannot load, at once unless an app is using it, otherwise from the next restart | The webcam |
| `ame security block-automount` | USB sticks and disks mounted only when you open them | A click in Files |
| `ame security block-user-extensions` | No GNOME extensions installed from extensions.gnome.org; the report lists the ones you have | The image's own extensions are not affected |
| `ame security block-xwayland` | No X11, from your next login | Steam, Wine, Proton and older apps |
| `ame security flathub-verified` | Only Flathub apps whose makers Flathub verified | Installed apps from anyone else stop updating |
| `ame security flatpak-password` | Installing or removing an app for the whole machine asks for an administrator's password | A password in Bazaar, and when uninstalling from an app's menu |
| `ame security noexec-temp` | Nothing started from `/tmp`, `/var/tmp` or `/dev/shm`, the first and last at once and all three from the next restart; a script run through its interpreter still runs | Some installers, build scripts and `go test` |
| `ame security no-coredumps` | No copy of a crashed program's memory on disk | Crashes cannot be diagnosed from one |
| `ame security package-cooldown` | npm, pnpm and uv refuse versions published in the last 3 days; `strict` waits 7 and runs no install scripts, and `on` goes back to 3. From new terminals, and for apps from your next login | A fix published today waits too; strict breaks packages that build native code |
| `ame security brew-attestations` | Homebrew checks each bottle's build attestation | `gh` and `gh auth login` on every account, or `brew install` fails |
| `ame security locked-poweroff` | The machine powers off once every desktop session has stayed locked for 18 hours, so that the disk keys leave memory. It goes by what the session reports, which a program already running as your account can reset | Whatever was open and not saved |
| `ame security audit-locked` | The kernel's audit log records what the file-watch sensor would, locked until the next restart, so that stopping the sensor cannot stop them | Every program a little slower. From the next restart |
| `ame security boot-password` | A password before anyone edits a boot entry or opens the boot menu's command line; `off` takes it away | Forget it and only a USB stick changes it. Not on a sealed image |
| `ame security vscodium-extensions` | VSCodium updates no extension by itself, and with names in `/etc/amethystora/vscodium-extensions` installs only those | Updates by hand. Developer mode only |
