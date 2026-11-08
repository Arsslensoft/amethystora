# Restarting

An update takes over at the next restart. Most of the time that restart takes seconds, your apps can come back
afterwards, and it can even happen at night while you are away.

## Restarting in seconds

Most updates change what runs on top of the kernel, not the kernel itself. When the waiting update keeps the kernel, its
initramfs, its settings and the SELinux policy as they are, **Restart now** in System Updates finishes it with a *soft
reboot*: your apps close, the system starts again from the new version, and the login screen is back in a few seconds,
without the firmware, the boot menu or the disk's passphrase. System Updates says when an update can be finished that
way, and asks before it closes your apps.

![A restart in seconds](media/soft-reboot.webm)

```bash
ame update restart
```

### When the computer restarts fully

The computer itself restarts for an update with a new kernel, one that brings another version of a driver in use, one a
firmware update is waiting behind, and on a [sealed image](streams.md#sealed-images); `ame update restart` then says
why. **Restart** in the system menu always restarts the computer. Either way the whole update takes over at once.

- An app that asks not to end the session, for a document not yet saved, still gets its say: the restart is left to
  GNOME's own dialog, which names it.
- The disk stays unlocked through a soft reboot, since the kernel keeps running: to have it locked again, shut down.
- The [live pill](pill.md) says **Ready** when an update waits for the restart; its **Restart now** opens System Updates.

## Picking up where you left off

When an update restarts the computer, from **Restart now**, `ame update restart` or a
[restart while you're away](#restarting-while-youre-away), the open apps and where their windows were are kept. At the
first login after it, the clock in the top bar says **Reopen your apps**: **Reopen** opens each on its workspace, in its
place, as a [saved set](workspaces.md#saved-sets) does. **Not now**, or ten minutes, forgets it; it is offered once.

![Apps come back](media/reopen-apps.webm)

Only the apps and their places are kept: never a window's title, a file, a page or a tab, so each app opens as from its
launcher (Notes opens locked). The note is a small file of your account's, deleted as soon as the next login has read
it. `ame desktop resume off` keeps nothing; **Pick up where you left off** in [Control](control.md) is the same switch.

## Restarting while you're away

An update waiting for its restart can be finished at night. Choose a time between 02:00 and 05:00 in
[Control](control.md) (**Restart while you're away**) or with `ame update night-restart 03:00`, and System Updates
offers **Tonight at 03:00** beside **Restart now**. At that time the computer restarts only if nobody would lose
anything:

- the screen is locked;
- no app asks not to end the session, for work not yet saved;
- it is on mains power;
- and it is awake: a computer asleep then stays asleep, nothing wakes it.

Otherwise nothing is closed, a notification says why, with **Tonight again**, and System Updates says the same until
the update is done. **Not tonight** takes it back. No password is stored and nothing unlocks: the restart is the one
**Restart now** makes, and the apps come back at the next login as above.

| Command | Does |
| --- | --- |
| `ame update night-restart 03:00` | Offer Tonight at that time (`02:00` to `05:00`), or `off` |
| `ame update tonight` | Restart tonight at that time, or `off` to take it back |
