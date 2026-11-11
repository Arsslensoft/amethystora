# Security

Amethystora is hardened beyond Fedora's defaults, and says so plainly. Everything below can be checked,
and every part of it can be turned off if it gets in your way ([Switches](security-switches.md#switches)).

## See where you stand

Open **Security** from the app grid, or run `ame security status`. Both show what the security settings
are doing right now and what to do about anything that is off. Neither changes anything or asks for a
password.

![Fix what needs you](media/security-tiles-green.webm)

In the app, what needs your attention comes first, with the command that fixes it and a button that runs
it in a terminal. The protections that are off until you want them are each one button away, and
**Check again** reads the report anew.

```bash
ame security status     # the report, in the terminal
ame security audit      # Lynis: a few hundred checks, each finding explained
```

### Opening the app on a page

`amethystora-security` followed by `scan`, `network`, `apps`, `containers`, `events` or `settings` opens
the app on that page; `settings/logins`, `settings/devices` and the like open one category of the
settings.

### Apps and containers

The **Apps** page lists what each Flatpak app can reach beyond its sandbox, and which of it was granted
on this machine ([App permissions](app-permissions.md)).

The **Containers** page lists what is installed outside Flatpak: the containers `amepkg` made, the apps
in your app grid that come from them, and what was built from the AUR, each with when it was last
upgraded. The report says so when one has gone two weeks without an upgrade, and **Upgrade them now**
upgrades every one in a terminal ([Containers](containers.md)).

### Asking the agent

When something needs your attention and the [AI agent](ai-agent.md) is on, **Ask the agent** hands it to
the agent, which looks into it in a terminal and changes nothing until you agree.

## On from the start

These hold on every machine from the first start. Each can be switched off
([Switches](security-switches.md#switches)).

### Signed updates

Only images signed with Amethystora's key are installed, so a tampered or substituted image is refused
([Updates](streams.md#signed-updates)).

### The firewall

Incoming connections are refused. Allowed in are printer and device discovery, Windows file sharing and
IPv6 setup. Your Tailscale tailnet is trusted, so who on it may reach this machine is decided by the
tailnet's own access rules (ACLs).

To let something else in, such as a development server you want to reach from your phone, use the
**Firewall** app or `sudo firewall-cmd --add-port=8080/tcp` (add `--permanent` to keep it after a
restart). `ame security firewall-away on` answers nothing at all on networks you have not named home
([Stricter settings](security-switches.md#stricter-settings)).

### Your phone, only at home

The phone link, which pairs your phone, is open only on the networks you name home, and closed on every
other, a café's or a new one included. `ame security home-networks` names them, from the networks this
machine has joined. A wired connection is used on every wired network the machine is plugged into, so
name it home only if the machine is plugged in nowhere else. A Wi-Fi network with no password cannot be named
home: any access point set up with its name would join it, so a home Wi-Fi is only as private as its password. The
same networks keep the machine's real
address when `ame security mac away` is on
([A different address on each network](network-security.md#a-different-address-on-each-network)).

### Wrong passwords

Ten wrong passwords within a quarter of an hour lock an account for ten minutes. Unlock it early from
another administrator account with `sudo faillock --user <name> --reset`.

Repeated failed logins over the network get the address blocked: five failures within ten minutes cost
an hour, and each further ban of the same address lasts longer, up to a day. It watches the SSH server,
the only thing here that can be logged into over the network, and leaves the tailnet alone.

```bash
ame security addresses             # what is blocked
ame security addresses <address>   # let one back in
```

### SSH

The SSH server is off, and refuses root logins when you turn it on. If you log in with a password,
[one-time codes](logins.md#one-time-codes-for-ssh) can ask for a code from your phone as well.

### Keyboards stay private

Applications cannot watch each other's keyboards. Flatpak apps are refused X11 and input devices
([Installing software](software.md#what-flatpak-apps-may-not-do)), and the key remapper, which reads
every key, is off until you ask for it.

### The kernel

Memory is wiped as it is handed out, kernel addresses are hidden, programs cannot read each other's
memory, and only a program's own debugger can write over its code. Rarely used modules (old network
protocols and filesystems, FireWire, test drivers, CAN, the floppy and parallel ports) cannot load.
`gdb -p` on a process you did not start needs `sudo`.

### Less to follow you by

Each network sees a temporary IPv6 address that changes, and Bluetooth a random address, visible and
pairable for 30 seconds when you ask for it.

### Lockdown

The kernel refuses to be rewritten while it runs: no modules without a signature it trusts, no writing
to `/dev/mem`, no BPF programs that write into another program's memory. A compromise of root ends at the
next restart. Programs that only read, such as `bpftrace`, keep working.

With Secure Boot on, the kernel locks itself down at every start. Without it, the image turns lockdown on
with a kernel argument, but the modules built into the image, such as DisplayLink's and the Xbox
controllers', cannot load then ([Hardware](hardware.md#secure-boot)). Where Secure Boot cannot be turned
on, `ame security lockdown off` leaves the argument out and `ame security lockdown on` puts it back, from
the next restart. The NVIDIA images leave the argument out, and on a sealed image it is part of the
signed kernel and cannot be changed.

### Sealed images

A sealed image checks every file of the system as it is read. Its kernel carries the signed digest of
the whole system, and a file changed on the disk cannot be read at all, whoever changed it
([Sealed images](streams.md#sealed-images)).

### What would survive a restart

Changes to accounts, to who may use `sudo`, to what starts by itself (your autostart entries, your own
systemd services, the shell startup files) and to the security settings, and every program that opens an
input device, are recorded. The security watcher reads them every 15 minutes and says what changed and
which program changed it ([The security watcher](security-watcher.md)).

On most machines a small sensor records them where they happen, at no cost to the rest of the machine;
where the kernel will not run it, the kernel's audit log does the same, which makes every program a
little slower. Something with root can stop the sensor: that is said too, but what happens while it is
down is not recorded, whereas the audit log, locked at boot, holds until the next restart.
`ame security audit-locked` keeps the audit log doing it if you would rather.

### Scans and audits

A virus scan runs every week, on Sunday night, over your home and temporary folders, and Lynis audits the
settings every month. By default neither deletes or moves anything: a false positive that takes away a
file you wanted is worse than most of what it would remove.

```bash
ame security scan       # what the last scan found
ame security scan now   # run one
```

[Scanning for viruses](viruses.md#scanning-for-viruses) does the same in a window, and
[Settings](security-settings.md) makes scans quarantine or delete what they find.

## Worth turning on

## The chip under the system

Most Intel computers have a second, small processor in the chipset, the Management Engine (ME), with
firmware of its own in the board's flash. It runs below the operating system whenever the board has
power, and no operating system, Amethystora included, can turn it off. AMD's processors have a chip of
the same kind.

### Active Management Technology

On business machines sold as **vPro**, ME carries Active Management Technology (AMT), which lets an
administrator control the computer over the network, even switched off. AMT's traffic is taken off the
network card before the system sees it, so the firewall here never sees it either. There is no
documented case of ME spying on the people using a computer, but there have been serious flaws in it,
most of them in AMT.

A machine has AMT if it has a vPro sticker or says vPro in its specifications, or if its firmware setup
(the menu a key opens as it starts) has an **Intel AMT** or **MEBx** entry. On such a machine:

1. **Set a firmware setup password, and change the MEBx password.** MEBx's password is `admin` until it
   is changed, and anyone with half a minute at the machine can use it to set up AMT for themselves, past
   the disk's encryption and a TPM PIN. This holds whether you keep AMT or not, so do it first.
2. **Turn AMT off** in the firmware setup, unless your organisation manages the machine with it.
3. **Block ports 16992 to 16995, 623 and 664** to the machine on your router, so that AMT cannot be
   reached from the internet. It does not help against someone on the same network.

### Keeping ME up to date

On every Intel machine, ME's fixes come as [firmware updates](updates.md#firmware) from the computer's
maker, which **Updates** installs; `fwupdmgr security` names ME's version and says whether it has known
flaws. A computer its maker no longer updates keeps the flaws it has, and the oldest chips (before the
10th generation) have one in ME's boot code that no update can fix.

### Disabling ME

Some computers are sold with ME disabled as far as it can be (System76, Purism, Tuxedo and Minifree sell
such machines). Disabling it yourself means rewriting the board's flash, which can leave a computer
unable to start and can take its firmware TPM with it, so Amethystora offers no command for it. It does
not block ME's drivers either: they bring protected video playback, Intel Arc cards' firmware updates,
and fwupd's reading of ME's version, which is how you learn that ME needs an update.

### What talks to it

Amethystora installs nothing that talks to AMT from the computer itself (Intel's LMS). The keyboards and
input methods it comes with, for Chinese, Japanese, Korean and the rest, send nothing of what you type:
Chinese Pinyin's cloud suggestions are off, and Fedora builds it without the network code they need.
