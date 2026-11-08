# Rolling back

The previous system is always kept beside the new one. If an update brings a problem, you can start the one before it,
and your files and settings are the same in both.

## Going back to the previous system

At the boot menu, pick the second entry: the system before the update. Nothing is changed, and the next restart goes
back to the newest. A notification at login says you are on the previous version and opens System Updates, which offers
**Keep this version** (makes it the one the machine starts; asks for your password) or **Restart into the latest**.

![The second entry at the boot menu](media/boot-menu-rollback.webm)

To stay on the previous system from a terminal, make it the default and restart:

```bash
sudo bootc rollback
systemctl reboot
```

### Staying there until a fix

The next update brings the newest version back, automatic or not. To stay where you are until a fix is out, turn
automatic updates off in System Updates, or [hold on to the build](#holding-on-to-one-build) you are on. Then
[report the problem](troubleshooting.md#reporting-a-problem), and follow the stream again once a fixed image is out.

## If an update does not start

The computer can go back by itself when a new system does not start, so nobody has to find the boot menu at the worst
moment. It is off until you turn it on, in Control's **If an update does not start, go back** or with
`ame update boot-guard on|off`, since it writes to the boot partition and has not been out long.

![Three tries, then back](media/boot-guard.webm)

### How it counts

After an update, the boot menu counts the new system's tries: three, or five on an encrypted disk, since powering off at
the passphrase counts as one. A start is good once the desktop is up (a login screen or a session on the screen); the
count is then gone, so a computer that started fine has nothing left that could send it back. Switching off before the
desktop is up counts as a failed try, on purpose or not. With no tries left, it starts the system it ran before, makes
that the default, and a notification says so at login.

### While it waits, the apps wait too

The automatic update then waits for a newer image than the one that did not start, and since it is one update for the
system, Flatpak apps and Homebrew, their updates wait with it, for days at most. To go on at once, press **Update now**
in System Updates (the notification has **Open System Updates**) or run `ame update`: they update the apps, and try the
image that did not start again if it is still the newest. `ame update boot-guard off` also lets the automatic update go
on.

### What it does not cover

Only the system is swapped: your home, apps and settings are not touched, and the failed update stays as the second
boot menu entry until the next one replaces it. It does not run where `/boot` is mirrored or on btrfs (the boot menu
cannot write its count there), nor on a [sealed image](streams.md#sealed-images). A system that starts but misbehaves
after login is not seen as failed: [roll back](rolling-back.md) by hand then.

## Holding on to one build

Every build also has a tag of its own, such as `stable-44.20260922`, from the version in the release notes. Switching to
it ([switching streams](streams.md#switching-streams-and-images)) holds the machine on that build while you wait for a
fix. It then receives no system updates, while apps and command line tools still update; System Updates and
`ame security status` remind you, and the second prints the command that goes back to following the stream.
