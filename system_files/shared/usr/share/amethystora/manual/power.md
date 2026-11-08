# Power you can see

Why is the fan loud, and why did the battery go so fast? On a laptop, the battery tile's menu opens on the answer:

- how full the battery is and when it will be empty or full, the watts it gives or takes now, and when it was unplugged;
- the apps that used the most since it was last plugged in or out;
- the battery's health: how much it holds against when it was new, and how many times it has been charged, when the
  battery says.

The apps are an estimate from the processor time each used, and the menu says so: the screen and graphics are not
counted, nor an app that came and went in between. Nothing is measured while the menu is closed but one look when you
log in and each time you plug in or unplug.

```bash
ame desktop power battery on|off
ame desktop power busy on|off
ame desktop power devices on|off
ame desktop power flat on|off
ame desktop power sleep on|off
ame desktop power charge on|off
```

Each is also in [Control](control.md).

## A busy app

When the processor has been at 70% or more for two minutes, a chip beside the meters names the app that kept it busy,
such as **Firefox is busy**. Click it for **Show it**, **Not now** and **Quit Firefox**, which asks the app to quit as
its own menu would. It is said once per app per hour, never while a window is fullscreen in front (a game), and goes
when the processor calms down. It rides the meters' own reading, so it still reads the processor with its meter off the
bar.

![A busy app](media/busy-app-chip.webm)

## Your devices' batteries

The same menu lists, under **This computer**, every device that says how full it is: a wireless mouse or keyboard,
headphones, a game controller, a pen, your paired [phone](wireless.md#phones).

- A row says **charge it soon** under 20%, and a notice says once when a device is at 15% or less (again only after it
  has been charged).
- On a computer without a battery of its own, the tile shows as **Batteries** while such a device is there, naming the
  emptiest.

It is what UPower already knows, followed as UPower says it changes: nothing is asked on a timer.

## A flat battery

At 3% the laptop does not simply die. A card says **Closing safely in 60 seconds**, and the pill shows the charge in
red. It cannot save an app's work for you, so the 60 seconds are yours. Which apps were open and where is kept at once,
and the computer powers off when the time is up or at **Close now**. At the next start the pill offers **Reopen your
apps**.

- **Not yet** holds it until the charge falls further; the card takes no typing, so `Super+Shift+N` says it from the
  keyboard while the card shows. Plugging in ends it.
- It closes rather than hibernates, since [kernel lockdown](security.md) refuses hibernation.
- If an app holds the logout, GNOME's dialog names it and asks; if you then go on, what was kept stays only until you
  plug in.
- Below 2% the system closes by itself, as it always did, and that is also what closes it while the screen is locked;
  a lock on battery keeps the same list, and the unlock takes it away.
- Only app names and window places are kept, never a title, a file or a page. A call or a recording in the pill keeps
  its place there; this one is listed in its card.

## What happened while it slept

Open the lid to a much emptier battery and a notice says how much went and what it can find out about why: how many
times the computer woke by itself and when, and what fired before the wake that was yours, which may not be the whole
story. It never names the key or mouse that woke it for you, a USB hub or a controller; when it cannot tell, it says it
did not say why, never that nothing woke it. Times need an administrator's account, since only those can read the
system's journal.

- **Stop it waking the computer**, offered for a USB or Bluetooth device or a network card, keeps that device from
  waking it, after an administrator's password. A mouse or keyboard kept from waking it needs the power button or the
  lid.
- **Undo**, `ame system wake-device on ID`, or the entry in [Control](control.md) under System puts it back
  (`ame system wake-device` lists them).
- It counts as unusual from 10 points gone, faster than 1.5 points an hour, over at least half an hour's sleep, and
  nothing is read unless it is. Each device is said once, and a loss with nothing to name once a day.
- What is kept is a salted checksum of what was said, in `~/.local/state/amethystora/sleep-report.json`: no name, ID
  or address of anything.

## Charged by the time you leave

Off until you turn it on. A battery that sits full all night wears faster, so on a laptop whose battery takes a charge
limit, plugged in for the night it **waits at that limit and finishes 75 minutes before you usually leave**. The pill
says **Held at 80%** and when it will be full; **Full now, just this once** in its card charges it now, until you next
unplug.

- **How it learns.** Each time you unplug after a night on mains it keeps the minute of the day and whether it was a
  weekend (the last thirty, nothing else: no date, no place), **sealed in your [Vault](vault.md)** under its own key in
  `~/.local/state/amethystora/charge-pattern.avs`. It needs three of a kind, and an earlier event in your calendar (its
  start time only, never its title) moves the time earlier.
- **The Vault.** Nothing asks for the Vault by itself, so after you log in the pill says once that it is **charging
  fully** while the pattern is sealed, and **Use my pattern** opens it. What it learns is sealed at the end of each
  night the Vault is open, the first night too; with the Vault closed it is kept in memory, and the pill says so once,
  with **Keep it**.
- **Always safe.** The time it works out is only ever in memory: a restart or a closed Vault means the battery charges
  fully. A limit you set yourself (in Settings) is never touched; it looks the first time it can, and a limit already
  there, set by a vendor tool or the firmware, counts as yours.
- **It gives back what it set** when you unplug, turn it off, go to sleep, shut down or log out, and when the release
  time comes, by the clock, even after the computer slept. If the system refuses to take it off, the pill says so and
  it tries again at the next change and the next start.
- **Where it is offered.** Only where `/usr/libexec/amethystora-charge-limit support` finds a battery with a
  `charge_control_end_threshold` that root may write (ThinkPads, many ASUS, Dell, Framework and others; a laptop
  without one never offers it). The limit is moved through UPower, which asks the system's own permission; if a laptop
  does not accept the change, the pill says so and nothing is held.

## Get me through

At 20%, the pill's question offers **Get me through**, and its card says what that changes: the power saver, the
built-in screen at 60% of what it was, and the backup and containers' upgrade waiting, for a week at most (a backup to
a disk in this computer never waits). The system update waits on its own only on a battery under 20%. It never comes on
by itself, and an update or backup you start yourself still runs.

The pill then says **Lasting until about 17:20**, from the battery's own estimate, so it is only that; its card also
says how long the battery said when you pressed the button, which is as much as it can say of how long this bought.

- **Back to normal.** The moment you plug in, or at **Back to normal** in the card, the power mode and the screen go
  back (the screen unless you moved it since), and what waited runs at its next turn. It also ends if you choose
  another power mode by hand, and after a lock if the machine was plugged in meanwhile.
- **If the session ends first** (the battery runs out), the screen's brightness is put back at the next start, from a
  small file kept in your home until then.
- **Not dimmed** while a call, a recording or a share, the microphone or the camera is on (with the privacy dots off, a
  call on this computer cannot be told, and the card says so); the power saver is held in any case.

`ame desktop get-through off`, or **Get me through on a low battery** in Control, makes the question offer power saving
alone.
