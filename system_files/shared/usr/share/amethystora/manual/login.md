# Logging in and locking

What you see when the screen locks, the first time a new account logs in, and each time a session starts.

## The lock screen

A large clock and the date, what is playing with its controls, and your notifications as one row per app with how many
it has: never what they say. To hide even those rows, turn off **Lock Screen Notifications** in **Settings**,
**Notifications**.

![The lock screen](media/lock-screen.webm)

The lock screen runs no extension, so it keeps Amethystora's own colours whatever the theme. With **High Contrast** on
(Settings, Accessibility), it is GNOME's own high-contrast lock screen instead, with GNOME's clock and plain cards,
which are easier to read.

## The first login

A new account's first login says hello in every language Amethystora speaks, the word written as if by hand in the gem's
light, your own language first, then goes on to [Pick a look](welcome.md). It happens once, and any key or click goes
straight on.

- With reduced motion, or animations off, the words stand still together and nothing is written.
- An account that has logged in before is never greeted by itself, and a dialog that opens over it (a password prompt,
  say) ends it.
- Only a small mark that it has been shown is kept, `~/.local/state/amethystora/hello.json`.

```bash
ame desktop hello show      # see it again (not over a dialog that is open)
ame desktop hello off       # a new account of this setup is not greeted
```

## The reveal at login

When a session starts, the screen begins where the boot splash ended: its dark sky, and the gem in the middle. The gem
swells a little and flies into the top bar, where it becomes the Amethystora logo, while the desktop shows through and
the top bar settles into place, all in about a second.

![The reveal at login](media/login-reveal.webm)

It happens each time you log in, not when you unlock the screen, and not at all with animations off (Settings,
Accessibility). Turn it off on [Control](control.md)'s **Appearance** page, or with
`ame desktop transition reveal off`.
