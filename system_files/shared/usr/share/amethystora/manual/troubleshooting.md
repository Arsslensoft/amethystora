# Troubleshooting

Most problems on an image-based system have the same three answers: find out what happened, roll
back if an update caused it, and report it so the next image fixes it.

## Let the agent look first

With the [AI agent](ai-agent.md) on (`ame agent toggle on`), **Diagnose a problem** in the Amethystora
menu (`Super+Alt+Space`), or from a terminal:

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

It reads the logs and crash reports, changes nothing, and comes back with the cause and a fix to
consider. [The AI agent](ai-agent.md#diagnose-a-problem) has more.

Without the agent, `ame system doctor` runs the same health check it starts from: failed services,
updates, disk space, memory, crashes, the clock, the driver, backups and pools of disks, each finding
with what fixes it. It only reads, needs no password, and `ame system doctor ID`, with the id a finding
named, checks that one again after a fix.

## The logs

Open **Logs** from the app grid, or run `amethystora-logs`. It opens on **Important**, the errors
from everything on the machine since it started. The sidebar has **All** of it, and splits it into your
session, the system, the kernel and hardware, security (sign-ins, `sudo`, what the audit rules or the
file-watch sensor caught) and **Crashes**; Important and Crashes say how many there have been since the
machine started. `amethystora-logs` followed by `all`, `session`, `system`, `kernel`, `security`,
`crashes` or `sources` (Apps and services) opens it on that view. Logs takes the place of GNOME Logs,
which the image no longer installs; Bazaar still has it.

- **Search** (`Ctrl+F` or `/`) looks through what the messages say, as you type, whatever their case.
  The `.*` button beside it takes the search as a regular expression instead.
- The newest 400 entries show first, and **Show older** reads further back. `Up` and `Down`, or `j` and
  `k`, go from entry to entry, and `Esc` empties the search, then closes the open entry. A view with
  nothing in it offers **Look further back** and **Every level**.
- **Time** goes back to the previous boot, any boot the journal still keeps, the last hour, day or
  week, or everything. **Level** hides what is less serious than you pick.
- **Apps and services** lists everything that has written to the journal. Pick an app to read what it
  said each time it ran, or a service of the system or of your session; the field at the top finds
  one, and the foot says how much room the journal takes on disk.
- Click an entry to read all of it, with every field the journal keeps. **Only …** narrows the view to
  where it came from, and **Around this** shows everything logged in the minute before and after it.
  With the [AI agent](ai-agent.md) on, **Ask the agent** on a crash or on what a service wrote has it
  find out why, changing nothing.
- **Follow** adds new entries as they are written, for a time that runs up to now. **Export** saves
  what is shown, up to 250,000 entries, to a text file or to JSON with one entry a line, and the terminal button opens the same view in `journalctl`. The command for the view is also
  at the bottom of the sidebar.

Administrators, the accounts in `wheel`, see everything. Any other account sees its own session and
apps only, and the views of the system say so.

The same in a terminal:

| Command | Shows |
| --- | --- |
| `journalctl -b -p warning` | Warnings and errors since this boot |
| `journalctl -b -1 -p warning` | The same for the previous boot, after a crash or a freeze |
| `journalctl --user -b` | Your own session: the desktop, your apps |
| `systemctl --failed` | Services that failed to start |
| `coredumpctl list` | Programs that crashed |
| `ame system logs-this-boot` | Everything from this boot |
| `ame system logs-last-boot` | Everything from the previous boot |
| `ame system local-overrides` | The files in `/etc` you, or something you ran, have changed |

## An update broke something

Pick the previous system at the boot menu to confirm it was the update. System Updates then offers to
keep it, which `sudo bootc rollback` does too, until a fixed image is out. [Updates](updates.md#rolling-back).

## Common problems

### A package I layered is gone

It was layered with `rpm-ostree install`, and the machine was then switched with `bootc switch`,
which builds the new system from the image alone. Install it again, and switch images or streams with
`rpm-ostree rebase` from now on ([Installing software](software.md#layering-the-last-resort)).

### An app cannot see a folder, a device, or will not open at all

Flatpak apps only reach what they are allowed to, and on Amethystora that excludes X11 and input
devices. Open **Flatseal**, pick the app, and grant what it needs: a folder under **Filesystem**, or
**X11 windowing system** for an app that does not speak Wayland.

### An extension misbehaves, or the desktop looks wrong

Turn the extension off in **Extension Manager**, then log out and back in. If the desktop still
misbehaves, put your account's settings back as the image ships them:

```bash
ame system reset-settings account
```

That resets the dock, your custom hotkeys, your extension choices and the theme, and keeps your
language, keyboard and accessibility settings. It saves them first, so `ame system reset-settings undo`
brings them back ([Resetting settings](#resetting-settings)).

### `Super+Ctrl+Space` changes nothing

It moves to the next wallpaper of the current theme. A theme of your own with a single picture has
nothing to move to, and a notification says so: add more ([Themes](themes.md#wallpapers)). The theme
picker is `Super+Ctrl+Shift+Space`.

### The account is locked after wrong passwords

Ten wrong passwords lock the account for ten minutes. Wait, or unlock it from another administrator
account with `sudo faillock --user <name> --reset`.

### A DisplayLink dock shows nothing

The screens on a DisplayLink dock or adapter need its driver, evdi, which the image carries, and
`displaylink.service`, which starts when the dock is plugged in. With Secure Boot on, the driver loads only
once Amethystora's key is enrolled: `ame security secure-boot`, then restart
([Hardware](hardware.md#secure-boot)). Until then the dock's screens stay dark, and
`systemctl status displaylink.service` says the service was skipped because its condition failed. With Secure
Boot off, the key cannot help: `ame system drivers` says what stands in the way on this machine, and what to run.

### Switching keyboard layouts

`Super+Space` is search here, so the next keyboard layout is on `Super+Shift+Space`. Add layouts in
**Settings → Keyboard**.

### The disk is filling up

```bash
ame system clean
```

Asks before each step: the container images no container uses and unused volumes, Podman's and
Docker's; the Flatpak runtimes no app needs; rpm-ostree's temporary files and cached metadata; and what
Homebrew no longer needs, with its download cache. It removes no container and no deployment: those
are `podman rm` and [rolling back](updates.md#rolling-back)'s business.

When the disk actually runs low, GNOME's notice says it for you: **Nearly out of room**, how much can be cleared, and that
none of it is your work, with **Show me first** and **Clear**. That is runtimes no app uses, Homebrew's old versions, container
layers without a name, the caches of Flatpak apps that are not running, thumbnails, and what your Privacy settings already
let go from the trash: older than the days you chose there, and never younger than 30. With removing old trash turned off there,
it takes nothing from the trash. It never takes a document, a container's volume or a named image. `ame system free-space`
does the same in a terminal, whenever you like. With the hourly snapshots on, what was in your home folder comes free only as
they expire, within 14 days, and the notice says so. `ame desktop low-space off`, or **Offer to clear space when the disk runs
low** in [Control](control.md), leaves GNOME's own notice.

### A disk of a pool failed

The notification names the pool and the disk, and says whether every file is still there. **Open
Control**, or run `ame system raid`, and replace the disk with a spare one at least as large; a pool
that did not open at all can be opened with a disk missing first. [When a disk fails](hardware.md#when-a-disk-fails)
explains each step. For the reason, `journalctl -k -g btrfs` shows what the kernel said about the
disk, and `ame system raid status` what each disk is counting.

### The machine waits at the start with a disk dead

A system on two disks waits for both at the start, with no end, when one has died: the screen stays
on the boot logo, or on "A start job is running for /dev/disk/by-uuid/…". Turn the machine off and on
again and choose **Start with a disk missing** in the boot menu, then replace the disk
([The system's own disks](hardware.md#the-systems-own-disks)).

### Resetting settings

When settings, not files, are what went wrong, put them back as the image ships them and keep
everything else:

```bash
ame system reset-settings
```

It asks which:

- **Your account**: the desktop's settings, the dock, hotkeys, extensions, theme and window layout,
  and what apps keep in GNOME's settings. Your language, keyboard and accessibility settings stay.
  Extensions you installed stay installed, and are off until you turn them on again. No password.
- **This machine**: every file in `/etc` that differs from the image's own copy in `/usr/etc`, for every
  account. It shows each file first, then asks for an administrator's password. Services you turned on are off again, and the image's own ones you turned
  off are on. What the machine knows stays as it is: the accounts and how they log in, its disks, name,
  networks and printers, its language, time and keyboard, what it trusts, its hardware and virtual
  machines, and every setting [Security](security.md) makes, whose report says when one of those
  changed. Restart afterwards.
- **Both**.

`ame system reset-settings account`, `machine` or `both` skips the question. A [sealed](updates.md#sealed-images)
machine resets only your account's settings.

Your files, apps and containers stay either way. What a reset replaced is saved first, your account's in
`~/.local/state/amethystora/reset-settings` and the machine's in `/var/lib/amethystora/reset-settings`,
and **Undo** brings the last one back: `ame system reset-settings undo`, which asks whose, or
`undo account` and `undo machine`. **Reset settings** on
[Control](control.md)'s **System** page opens the same command.

### Starting over

When a machine has collected more changes than are worth undoing one by one, and
[resetting the settings](#resetting-settings) was not enough, it can start again as a new installation
of the image it runs:

```bash
ame system powerwash
```

It asks twice, then makes a new installation beside the current one, which starts at the next
restart: the image's own settings in `/etc`, an empty `/var`, and no accounts, home folders or settings
of yours. Nothing is erased. The old system stays on the disk, taking up its space, and the boot menu
can still start it, so nothing is lost if you change your mind; getting rid of it for good is a job
for an administrator in a terminal. It is experimental, and not offered on a [sealed](updates.md#sealed-images)
machine. To give a machine away, install it again from the ISO instead, which erases the disk.
**Start over** on [Control](control.md)'s **System** page opens the same command.

## Reporting a problem

Report bugs on [GitHub](https://github.com/arsslensoft/amethystora/issues), and ask questions in the
[discussions](https://github.com/arsslensoft/amethystora/discussions), which **Community** in the app grid
opens. `ame report` gathers what a
report needs into a file in your home folder (the image and its deployments, the kernel, the machine's maker
and model, what the [driver check](hardware.md#when-a-device-does-not-work) found, your groups, failed
services and the errors logged since this boot), shows you all of it, and then, if you say so, opens a new issue with all but
the errors filled in. Nothing is posted until you submit it; drag the file in if the errors help.
Otherwise, include:

- the output of `rpm-ostree status` (`sudo bootc status` on a sealed machine), which names the exact image,
- what you did, what you expected, and what happened instead,
- the relevant lines from the logs above,
- whether the previous image had the same problem.
