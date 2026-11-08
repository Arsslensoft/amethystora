# The live pill

While something is happening, the clock in the middle of the top bar grows into a small round pill that shows it. When
nothing is, it is the clock again, exactly as before.

![The pill grows](media/pill-grows.webm)

Click the pill for its card: its buttons and, below them, whatever else is happening. The card's last row is the date
and time, which opens the calendar; a right-click on the pill, or `Super+V`, always opens the calendar.
`ame desktop pill off` gives you GNOME's clock alone; `ame desktop pill on` brings the pill back.

## What the pill shows

The first of these that is happening shows in the pill, and the second [beside it](#two-at-once):

- **A timer.** At the end of the calendar's column, **Start a timer** offers 1, 5, 10 and 25 minutes; while one runs,
  the same buttons add that many. The card adds a minute, pauses or cancels. At the end you hear it, a notification
  says so (even when banners are quiet), and the pill says **Timer done**. It runs on while the screen is locked, and
  rings as soon as you unlock if it ended meanwhile.
- **What is playing**, from any player that tells the desktop: the cover, title and artist, and a small pause button.
  The card has Back, Pause, Next and how far into the track you are. A player you paused stays for ten minutes.
- **A call** on your [paired phone](phone.md): who is calling, **Mute ringer**, and in the card **Reply with a text**.
  You answer on the phone itself.
- **A recording** of the screen: how long it has run, and **Stop**. GNOME's own recordings go to Videos › Screencasts.
- **A link from your phone**, shown and never opened by itself: **Open in browser**, **Copy**, or **Not now**, which
  leaves it in the notification list. A name in another alphabet is shown in its plain form (`xn--…`), so a
  look-alike cannot pass for the site it imitates.
- **An update ready**: **Ready · quick restart** when it can finish without restarting the computer, else **Ready ·
  restart**. **Restart now** opens [System Updates](updates.md#updating-now), which asks before it restarts anything;
  **Tonight** puts it out of sight until 8 in the evening.
- **Updated**, once, the first time you log in after an update: what it brought, and
  [What's new](updates.md#whats-new) in the Manual.
- **A meeting about to start**: see [Meetings in the pill](#meetings-in-the-pill).
- **The battery.** Rest the pointer on the battery in the top bar for the time left, or until full. At 20% the pill
  turns amber and offers [Get me through](power.md#get-me-through), once; it never turns it on by itself.
- **A disk still being written to**: see [A USB stick that tells the truth](#a-usb-stick-that-tells-the-truth).
- **Printing**: see [Printing](#printing).
- **A word that something just finished.**
- **Quiet**, and how many banners it holds: see [Quiet](notifications.md#quiet).

## Two at once

When two things are happening, the second sits as a small bubble beside the pill, with its icon and a few characters,
so music stays in sight during a timer. A click on either opens its own card. A call, a recording and Present keep the
place alone; anything else waits in the card.

![Two at once](media/pill-two-at-once.webm)

`ame desktop pill two-at-once off`, or Control's **Two at once**, shows only the first.

## The pill's weight

The pill overshoots a little and settles, gives a little when pressed, and banners grow out of it and go back in.
Reduce motion and the animation speed still rule, and the [light desktop](comfort.md#the-light-desktop) and power
saving have the plain motion. `ame desktop pill weight off` keeps the plain motion always.

## A timer you can feel

The ring drains round the pill itself, a second timer sits beside the first, and the end comes with a gentle glow as
well as the sound.

![A timer you can feel](media/pill-timer.webm)

Type `tea 4 min` in search and press `Enter` for a timer called Tea (`5 min` for one with no name). Its name is kept in
memory only. `ame desktop search timers off` takes that search answer away; `ame desktop pill timer off` goes back to
one timer with its small ring.

## Everything moving

Downloads, Flatpak updates, copies in Files, backups and files you send to your phone show as one thing in the pill,
with one ring, and its card lists each. The ring shows how far it has got where something says so, and is whole where
nothing does. What a terminal copies with `cp` says nothing to anybody and is not shown.

- File names stay in memory, and are not in the pill or its card while the screen is recorded or shared.
- What stops saying anything, or has gone on for hours, drops out by itself.
- `ame desktop pill moving off` turns it off.

While something moves, the computer does not go to sleep by itself, for twelve hours in a stretch at most. The screen
can still go dark, closing the lid still sleeps, and on battery it gives way below 15% (and takes hold again when you
plug in). Choosing Suspend yourself still suspends it, with no password. `ame desktop pill stay-awake off` lets it
sleep.

## Hello, headphones

Headphones, a speaker, a game controller, a phone or a dock arriving grow the pill for two seconds, with the device's
name and battery. It asks nothing, and says nothing during a call, a recording, [Focus](focus.md#focus) or quiet hours.
`ame desktop pill hello off` turns it off.

## Drop things on the pill

Drag a file up to the pill and it opens into targets: **Send to phone**, **Shelf**, **Convert**, **Copy without hidden
data** and **Ask the agent**. A target that is off or not installed is not there, and nothing happens until you let go
on one.

![Drop on the pill](media/pill-drop.webm)

`ame desktop pill drop off` turns it off.

## Music you can see

The pill and its card take the album's colour where the text still reads on it, and you can drag the position in the
card where the player can seek. A cover is read once per track, only from a file on this computer.
`ame desktop pill music off` turns that off.

The bars can follow the real sound of the player's own stream while they are on the screen, but that is **off** until
it is shown to cost no more than 1% of a processor: `ame desktop pill music-bars on`, or **Bars that follow the sound**
in [Control](control.md). The level is read and thrown away, never recorded. Reduce motion, power saving, the light
desktop, a blank screen, the battery and a player whose stream cannot be told keep the four beats.

## Printing

While a job of yours is in the queue the pill says **Printing**, with **3 of 12 pages** or **page 3** when the printer
gives those, and **Printed** at the end. When something is wrong it says so in words: out of paper, paper stuck, out of
ink or toner, a cover open, the printer not answering. Paper added or a cover closed carries on by itself.

- The card has **Cancel**, which clears your job, and **Try again** when the job needs a push. Starting a printer that
  has stopped takes an administrator's account; anyone else is offered Settings' Printers.
- A job unchanged for ten minutes (a printer left off, a job held on purpose) is asked about only now and then, and
  steps back behind music and updates, still in the card.
- Only your own jobs are followed, and the document's name is never asked for. GNOME's own printer notices stay.

`ame desktop printing off`, or **Printing in the pill** in Control, turns this off.

## Meetings in the pill

Five minutes before an event of today's calendar that has a video-call link, the pill shows it with a countdown,
*Design review in 4 min*, then *Design review now* for ten minutes or until it ends. Its card has the times, the
calendar, the link, **Later** and **Join**. **Join** opens the link in your browser; nothing opens until you click it.
**Later** puts it away until the meeting starts, and for good once it has.

The events are the ones the calendar menu shows, from calendars added in Settings › Online Accounts or in Calendar. The
link is read from the event itself (its conference link, location, link or description), and only a link that looks
like a call counts. Nothing is sent anywhere. `ame desktop pill meetings off|on`, or **Meetings in the pill** in
Control.

## A USB stick that tells the truth

A copy can say it is done while most of it is still in memory, on its way to the stick. While a removable disk is
written to, the pill says **Still writing to “Photos”**, with about how far it has got, and once everything is there,
**All on “Photos”**. Its card says about how much is still waiting, names what of yours is holding the disk, with
**Quit**, and has **Put it away**; if that is refused, the list says why. It says the disk is safe to unplug only when
the whole drive is put away, not just one partition.

### How sure "All on" is

The percentage and amount are estimates and say "about": Linux tells an account how much the whole computer has waiting
to be written, not how much is for this disk, so the number can include other writes, and it never goes down while one
copy goes on. "All on" is said only when all of this holds:

- the disk's file system was asked to finish (`sync -f`) and wrote nothing more while it did (a sync that wrote
  something is followed by another);
- the computer's waiting data did not rise during it or since;
- none of your programs holds the disk (else the pill says **Something is still using** it).

It can still miss another account's program that writes, and a copy of a few megabytes may finish before the pill
notices. A disk whose file system cannot be asked to finish from here, such as an NTFS stick mounted through
`ntfs-3g`, never gets "All on": it stays on **Still writing** until you put it away. A stick is asked to start writing
as soon as the waiting data rises, instead of half a minute later, at most once a minute; a disk that may be asleep is
left to the kernel.

### What it reads

While such a disk is mounted and you are at the computer, it reads one small number every 15 seconds (30 after a minute
of calm); the disk's own numbers only when the waiting data has risen or the disk's top folder has changed; each second
while the disk is busy. Nothing at all with no such disk, a disk mounted read-only, or after a minute with no input.
Nothing is written down. `ame desktop usb-writes on|off`, also in [Control](control.md).
