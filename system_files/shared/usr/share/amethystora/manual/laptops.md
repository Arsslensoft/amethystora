# Laptops

Laptops get their known fixes at first boot, keep their firmware current through Updates, and can say whose they are
on the lock screen.

## Framework laptops

Framework laptops get their known fixes at first boot:

- the keyboard on the Intel models, which turns the light sensor off too and restarts the machine once by itself;
- suspend on a Laptop 13 with AMD Ryzen 7040 and firmware older than 3.09, whose own keyboard then no longer wakes it;
- natural scrolling for a mouse;
- a larger text size on a Laptop 13 with its original 2256×1504 screen.

A Framework 16's keyboard and numpad modules cannot wake it.

## Keeping the firmware current

Keep the firmware current on every machine: a notification says when an update is waiting, and **Updates** shows what
there is. [Firmware](updates.md#firmware) says more. In a terminal:

```bash
fwupdmgr get-updates
fwupdmgr update
```

## Measuring power draw

`ame system power-draw` measures what the machine draws while idle, with powerstat: run it on battery, and leave the
machine alone for the few minutes it takes.

## If it's lost

A laptop left on a train has only a login screen to say whose it is. `ame system if-found`, or **If it's lost, it says
whose it is** in [Control](control.md)'s System page, writes one line of yours, 120 characters at most, on the login
screen and at the bottom of the lock screen, for every account: "If found, please call Sam", and a number. It needs an
administrator's password.

![A line on the lock screen](media/if-found.webm)

### What it shows, and to whom

- Anyone holding the computer can read it, so choose what it says. It adds no way in, no account and no tracking.
- A computer turned off with its disk encrypted asks for the disk's password first and shows the line only after that,
  so on a laptop the line is mostly seen when it was only asleep.
- The login screen shows a change the next time it appears, and an account's lock screen the next time that account
  logs in. It shows only what the administrator wrote, never a line a program of that account sets for itself.

`ame system if-found off` takes it away.
