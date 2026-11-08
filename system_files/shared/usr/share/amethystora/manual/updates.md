# Updates

Updates are automatic and quiet: the system, your Flatpak apps and your Homebrew tools are downloaded in the background
while you work. The new system is staged beside the running one and takes over whole at the next restart, so nothing is
ever half-updated and an update cannot leave the machine unable to start.

> **The one habit to keep:** shut down or restart when you are done for the day. That is when a
> staged update is applied.

![Updates in the background](media/update-background.webm)

## Updating now

[Show me](guide:update-now)

Open **System Updates** from the app grid (or **Check for updates** under **System** in the Amethystora menu, or
`amethystora-update`) and press **Update now**. It updates the system, the Flatpak apps of the machine and of every
account signed in, and your Homebrew tools in one go, showing each as it happens. It asks for no password and skips the
battery, busy-machine and memory checks that hold the automatic updates back. It is the same update the automatic ones
run, so if one is already under way the window follows it. Closing the window does not stop it.

### What System Updates shows

A new system still takes over at the next restart: once one is ready, System Updates says so and offers the restart.
It shows the version you are running, the one waiting for the restart and the one kept for
[rolling back](rolling-back.md). It also turns automatic updates off or on, which asks for an administrator's password.
**Release notes** opens them on GitHub; **Check again** looks once more when everything is up to date.

### Containers

If you have [containers](containers.md#keeping-them-up-to-date) from `amepkg`, **Update now** upgrades them straight
after the rest, under **Containers**. One that fails says so there, with
`journalctl --user -u amethystora-pkg-upgrade`; the system's update still counts as done.

### When an update stops

An update that does not finish says which part stopped; the rest is still up to date. **Try again** starts it once more,
and an automatic one tries again by itself a minute later. If a new system is ready anyway, **Restart now** is still
offered, and with the [AI agent](ai-agent.md) on, **Ask the agent why** has it look into it. **What the updater said**
shows administrators the updater's messages, with the `journalctl` command for all of them.

### In a terminal

```bash
ame update
```

| Command | Does |
| --- | --- |
| `ame update` | Update everything now |
| `ame update restart` | Restart to finish the update, in seconds when it can be ([restarting in seconds](restarting.md#restarting-in-seconds)) |
| `amepkg upgrade-all` | Upgrade the containers of other distributions now |
| `ame system auto-updates on` | Turn automatic updates on, or `off`; without either it asks |
| `ame update check` | Look for what is waiting, without installing it ([what is waiting](#what-is-waiting)) |
| `ame update check off` | Stop the daily look, or `on` again |
| `ame changelog` | The release notes of the build you are running, from GitHub (on a stream other than `stable`, the newest release's) |
| `rpm-ostree status` | The running system, the staged update and the one to roll back to (`sudo bootc status` on a [sealed](streams.md#sealed-images) machine) |
| `fwupdmgr get-updates` | Firmware updates for this machine, from its maker |

The release notes of every stable build are on [GitHub](https://github.com/arsslensoft/amethystora/releases).

## What is waiting

Once a day, and after each start, the machine looks for updates without installing anything: a newer version of its
image, and newer versions of packages you added with [`rpm-ostree install`](software.md#layering-the-last-resort)
(looked for every day, even when the image has not changed). System Updates lists them under **Waiting to be
installed**; **Check now** looks again without a password, and **Update now** installs them.

While [automatic updates](#updating-now) are off, a notification says when something new is waiting, once per version,
and opens System Updates. With them on, nothing is said: they install it themselves.

### What the look sends

It asks `ghcr.io` for the image and, only when packages were layered, Fedora's mirrors for their lists. On a connection
that counts data ([saving data](#saving-data)) the daily look waits for another; **Check now** looks anyway.
`ame update check off` stops it, and it is listed with the rest of what the machine sends by itself
(`ame security sends`). A [sealed](streams.md#sealed-images) machine does not look: its updates show once they are
ready for the restart.

### bootc, rpm-ostree and the kernel's arguments

`ame update` updates with `bootc`, or with `rpm-ostree` on a machine with layered packages, which `bootc` cannot update,
as the automatic updates do.

An update can also change the kernel's arguments (`/usr/lib/bootc/kargs.d` in the image). `bootc` applies them as it
stages the update; `rpm-ostree` leaves them out, so on a machine with layered packages Amethystora gives the staged
update the image's arguments right after staging and again just before the restart: what the new image adds is added,
what it drops is taken off, and your own stay. `/usr/libexec/amethystora-kargs status` says whether the waiting update
has them. Such an update always takes a full restart. A machine updated with `rpm-ostree` before this got the arguments
it was missing once, at the first start after the update that brought it, except one you turned off with
`ame security kernel-args` and kernel lockdown, which stays as `ame security lockdown` left it.

## Saving data

On a connection that counts data (a phone's hotspot, mobile broadband, or a network marked **Metered connection** in
Settings), the big downloads wait for one that does not: the automatic system update with your apps and Homebrew tools,
the daily upgrade of your containers, and a backup over the network. A backup to a disk of this computer still runs.
Nothing waits longer than a week; after that it runs on any connection.

Meanwhile the clock in the top bar says **Saving data**, and its card lists what waits and since when. **Update anyway**
opens System Updates and updates now, as **Update now** and **Back up now** always do, whatever the connection.

- `ame desktop saving-data off` keeps the pill quiet about it; the updates still wait.
- To have a connection count as unlimited, turn **Metered connection** off for it in Settings, under Wi-Fi or Network.

## Firmware

The firmware of the machine and its devices (the UEFI firmware, docks, SSDs, keyboards, the fingerprint reader) is
updated apart from the system, since it can need mains power and a restart. Every day fwupd looks on LVFS, where makers
publish it, and the **Firmware** section of **Updates** lists each device that has some, from which version to which.

### Installing firmware

A notification says so once per new release and opens **Updates**. Updates its maker marks as important for security
are marked **Important**, and the [security report](security.md#see-where-you-stand)'s **Firmware updates** check names
them too. **Update firmware…** installs in a terminal, where fwupd says what it will do and asks before it restarts the
machine; **Check now** asks LVFS again. Firmware for the UEFI firmware itself is written at the next restart: leave the
machine plugged in until it has started again.

```bash
fwupdmgr get-updates      # what there is
fwupdmgr update           # install it
fwupdmgr security         # how well the firmware protects this machine
```

`ame security firmware-metadata off` stops the daily fetch (LVFS is asked once a day at most, and nothing about this
machine is sent); Updates then lists no firmware, but **Check now** still asks.

### Firmware security

The security report shows fwupd's rating of the firmware's protections, from HSI:0 to HSI:5; how high a machine can
get is mostly up to its maker. Two findings ask for something to be done:

- **Revoked boot programs:** LVFS has a newer list of the boot programs Secure Boot must refuse (the UEFI dbx) than the
  firmware holds. `fwupdmgr update` installs it, and refuses while a boot program on this machine is one the list
  refuses.
- **Platform key:** the maker shipped a test key whose private half is published (PKfail), so anyone can sign what
  Secure Boot starts. Only the maker can replace it, with a firmware update.

## What's new

The first time you log in after an update, the clock in the top bar says **Updated · what's new** for a minute. Click
it for the first three things the updates since your last version brought, even if you skipped a few, and **Read it
all** for [What's new](whats-new.md) in this Manual. It is said once per update, and not at all for an update with
nothing new to notice, a new account's first version, or a machine started on an older version (after a rollback).

The page is written when the image is built, from the new features of the stream's last few builds, so it is always
about the version you are running; it is in English only. `ame desktop whats-new off` keeps the pill quiet after
updates; the page stays in the Manual.
