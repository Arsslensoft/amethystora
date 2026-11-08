# The security watcher

Every 15 minutes, or as things happen with [real-time watching](security-settings.md) on, the
watcher looks at what is listed below and tells you what changed and which program changed it.

On battery, its runs every 15 minutes wait for mains power, and the next one reads the audit log and the
journal from where the last one stopped; `ame security settings on-battery run` keeps them going on
battery too.

## The audit log

Accounts, `sudo` rules, what starts by itself (services, autostart entries, the shell startup files,
`~/.ssh`, GNOME Shell extensions, top-bar widgets, D-Bus services, udev rules, git's settings), the
security settings, SELinux's, and kernel modules loaded from a login, each with the program that made the
change.

A program opening the keyboard directly is said the first time that program does it; games and key
remappers do it too. When too many events come at once, the kernel drops some rather than slow the
machine down, and **Audit rules** in the report says how many it dropped in the last week.

## The journal

An account locked after wrong passwords, three or more wrong `sudo` or administrator passwords, and USB
devices [USB protection](devices-security.md#block-usb-devices-you-did-not-plug-in) blocked.

## /etc against the image

The image keeps its own copy of `/etc` in `/usr/etc`, so a changed security setting, or a file in `/etc`
that replaces one of the image's in `/usr/lib`, shows as **Image settings** in the report.

## What should not exist

**Rootkit checks** in the report: a setuid program outside `/usr` (searched for once a day),
`/etc/ld.so.preload`, a plain file in `/dev`, a file or folder in `/etc` or `/usr/local` that belongs to a
login account rather than to root, a kernel module that did not come with the image or is not signed, and
a network interface reading every packet.

## What accepts connections

Each program that listens on the network, which every device on your tailnet can reach. A new one is
said once.

## What your session finds first

A program in `~/.local/bin`, `~/bin` or Homebrew's folder named like one of the system's commands, which
your shell runs in place of the real one. A fake `sudo` or `ssh` there is an old way to catch a password,
so those are said loudest; Homebrew's folder only counts for those. Also a launcher in your app grid that
no container export made. Each is said when it appears, and again if it changes.

A folder the audit log watches, such as `~/.config`, made a link to somewhere outside your home folder is
said too, once: the audit log stops watching it.

## Network protection and the report

- **Network protection**, while it is on: each connection it blocked, with the way to allow it again,
  and what it recognised without blocking when the rule rates it serious
  ([Network protection](network-security.md#network-protection)).
- **The report itself**: when a check that says something was turned off, such as the firewall, signed
  updates, SELinux or the audit log, stops being fine, and when one of the protections that are off until
  you turn them on is turned off again. A change you make with its own command is recorded and not said
  ([Switches](security-switches.md#switches)).
- **Itself**: a run that failed, or none for an hour, is said by the report, and by the next run that
  finishes.

## Events

Anything new is a notification, and waits in the report until you read it. `ame security events now`
looks first. If you made the change, or installed something that did, there is nothing to do.

```bash
ame security events
```

Each finding keeps when it happened, which part of the watcher found it, the account it is about and the
kind of attack it would belong to, in `/var/log/amethystora-security-events.jsonl`. The newest 200 are on
the Security app's **Events** page, which only administrators see. **Mark them read, in a terminal** there
does what `ame security events` does, and **Open in Backups**, on an event about your own files, opens
Backups at the snapshot before it.

What it reads that somebody else wrote (a file name, a program's name, a launcher's command, a host name
from the network) is shown as plain text everywhere: a name cannot write over a terminal or a
notification, or pretend to be another line of the history.
