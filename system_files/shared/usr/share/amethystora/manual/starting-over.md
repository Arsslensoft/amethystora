# Resetting and starting over

Two ways back to a clean machine: reset the settings and keep everything else, or start again as a new
installation. Both keep what they replace, so you can change your mind.

## Resetting settings

When settings, not files, are what went wrong, put them back as the image ships them. **Reset settings** on
[Control](control.md)'s **System** page opens the same command:

```bash
ame system reset-settings     # asks which; account, machine or both skips the question
```

### Your account, this machine, or both

- **Your account**: the desktop's settings, the dock, hotkeys, extensions, theme and window layout, and
  what apps keep in GNOME's settings. Your language, keyboard and accessibility settings stay. Extensions
  you installed stay installed, and are off until you turn them on again. No password.
- **This machine**: every file in `/etc` that differs from the image's own copy in `/usr/etc`, for every
  account. It shows each file first, then asks for an administrator's password. Services you turned on are
  off again, and the image's own ones you turned off are on. Restart afterwards.
- **Both**.

A [sealed](streams.md#sealed-images) machine resets only your account's settings.

### What a reset keeps

Your files, apps and containers. A machine reset keeps what the machine knows: the accounts and how they
log in, its disks, name, networks and printers, its language, time and keyboard, what it trusts, its
hardware and virtual machines, and every setting [Security](security.md) makes, whose report says when one
of those changed.

### Undoing a reset

What a reset replaced is saved first, your account's in `~/.local/state/amethystora/reset-settings` and the
machine's in `/var/lib/amethystora/reset-settings`. **Undo** brings the last one back:
`ame system reset-settings undo`, which asks whose, or `undo account` and `undo machine`.

## Starting over

When a machine has collected more changes than are worth undoing one by one, and
[resetting the settings](#resetting-settings) was not enough, it can start again as a new installation of
the image it runs. **Start over** on [Control](control.md)'s **System** page opens the same command:

```bash
ame system powerwash
```

It asks twice, then makes a new installation beside the current one, which starts at the next restart: the
image's own settings in `/etc`, an empty `/var`, and no accounts, home folders or settings of yours.

### Nothing is erased

The old system stays on the disk, taking up its space, and the boot menu can still start it, so nothing is
lost if you change your mind; getting rid of it for good is a job for an administrator in a terminal. It is
experimental, and not offered on a [sealed](streams.md#sealed-images) machine. To give a machine away,
install it again from the ISO instead, which erases the disk.
