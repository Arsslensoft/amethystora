# Common problems

The problems people meet most often, each with its cause and what fixes it. For anything else, start with
[Troubleshooting](troubleshooting.md).

## A package I layered is gone

It was layered with `rpm-ostree install`, and the machine was then switched with `bootc switch`, which
builds the new system from the image alone. Install it again, and switch images or streams with
`rpm-ostree rebase` from now on ([Installing software](software.md#layering-the-last-resort)).

## An app cannot see a folder, a device, or will not open at all

Flatpak apps reach only what they are allowed to, and on Amethystora that excludes X11 and input devices.
Open **Flatseal**, pick the app, and grant what it needs: a folder under **Filesystem**, or **X11
windowing system** for an app that does not speak Wayland.

## An extension misbehaves, or the desktop looks wrong

Turn the extension off in **Extension Manager**, then log out and back in. If the desktop still misbehaves,
put your account's settings back as the image ships them:

```bash
ame system reset-settings account
```

That resets the dock, your custom hotkeys, your extension choices and the theme, and keeps your language,
keyboard and accessibility settings. It saves them first, so `ame system reset-settings undo` brings them
back ([Resetting settings](starting-over.md#resetting-settings)).

## `Super+Ctrl+Space` changes nothing

It moves to the next wallpaper of the current theme. A theme of your own with a single picture has nothing
to move to, and a notification says so: add more ([Themes](wallpapers.md)). The theme picker is
`Super+Ctrl+Shift+Space`.

## The account is locked after wrong passwords

Ten wrong passwords lock the account for ten minutes. Wait, or unlock it from another administrator account
with `sudo faillock --user <name> --reset`.

## A DisplayLink dock shows nothing

A DisplayLink dock or adapter needs its driver, evdi, which the image carries, and `displaylink.service`,
which starts when the dock is plugged in.

- **With Secure Boot on,** the driver loads only once Amethystora's key is enrolled:
  `ame security secure-boot`, then restart ([Hardware](hardware.md#secure-boot)). Until then the screens
  stay dark, and `systemctl status displaylink.service` says the service was skipped because its condition
  failed.
- **With Secure Boot off,** the key cannot help: `ame system drivers` says what stands in the way on this
  machine, and what to run.

## Switching keyboard layouts

`Super+Space` is search here, so the next keyboard layout is on `Super+Shift+Space`. Add layouts in
**Settings → Keyboard**.

## The disk is filling up

```bash
ame system clean
```

It asks before each step: the container images no container uses and unused volumes, Podman's and
Docker's; the Flatpak runtimes no app needs; rpm-ostree's temporary files and cached metadata; and what
Homebrew no longer needs, with its download cache. It removes no container and no deployment: those are
`podman rm`'s and [rolling back](rolling-back.md)'s business.

### When the disk runs low

GNOME's notice says it for you: **Nearly out of room**, how much can be cleared and that none of it is your
work, with **Show me first** and **Clear**. `ame system free-space` does the same in a terminal, whenever
you like.

- **What it clears:** runtimes no app uses, Homebrew's old versions, container layers without a name, the
  caches of Flatpak apps that are not running, thumbnails, and trash older than the days your Privacy
  settings chose, never younger than 30. With removing old trash turned off there, it takes nothing from
  the trash.
- **What it never takes:** a document, a container's volume or a named image.
- **With the hourly snapshots on,** what was in your home folder comes free only as they expire, within 14
  days, and the notice says so.

`ame desktop low-space off`, or **Offer to clear space when the disk runs low** in [Control](control.md),
leaves GNOME's own notice.

## A disk of a pool failed

The notification names the pool and the disk, and says whether every file is still there. **Open
Control**, or run `ame system raid`, and replace the disk with a spare one at least as large; a pool that did
not open at all can be opened with a disk missing first. [When a disk fails](disks.md#when-a-disk-fails)
explains each step. For the reason, `journalctl -k -g btrfs` shows what the kernel said about the disk, and
`ame system raid status` what each disk is counting.

## The machine waits at the start with a disk dead

A system on two disks waits for both at the start, with no end, when one has died: the screen stays on the
boot logo, or on "A start job is running for /dev/disk/by-uuid/…". Turn the machine off and on again,
choose **Start with a disk missing** in the boot menu, then replace the disk
([The system's own disks](disks.md#the-systems-own-disks)).
