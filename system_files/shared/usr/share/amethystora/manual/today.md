# Today and rest

The calendar menu (click the clock, or `Super+V`) holds your day; GNOME's rest reminders and a resting screen help you
step away from it.

## Today

Under the calendar: the events from your calendars, the tasks due today and the overdue ones from
[Notes](notes.md#tasks), your world clocks and the weather. On a touchpad, four fingers to the left open it and to the
right close it ([Touchpad gestures](classic-layout.md#touchpad-gestures)).

![Four fingers for Today](media/four-left-today.webm)

**Tasks from Notes** show only on today's page, while Notes is open and unlocked (or not encrypted); click them to open
Notes' tasks. Nothing is asked of Notes until you open the menu, and Notes tells the calendar only those tasks' titles,
and nothing while the menu is closed. Notes answers GNOME Shell alone: an app in a sandbox cannot ask, but a program you
run yourself outside one could, as it could read anything else of yours while your notes are open.
`ame desktop today tasks off` leaves them out.

### Getting set

For your first four weeks, the calendar menu starts with **Getting set**: the few steps that protect the machine and
make it yours. Pick a look, automatic updates, a backup of your home folder, a PIN to unlock the disk, and pairing your
phone.

- Each is ticked by what the machine already says (a backup ran, a phone is paired, the security report says the disk
  asks for its PIN), never by clicking it. A click opens the place that does it; the security line only opens
  [Security](security.md).
- A step the machine has nothing to do about (a disk that is not encrypted, say) is left out, and nothing is read until
  you open the menu.
- **Hide**, or `ame desktop getting-set off`, puts it away for good; once every step is done it goes by itself.

### Your year

From mid-December to mid-January, a card in Today offers **Your year, on this machine**: how long you have been
together, the apps you used most, and what was looked after. The days count from the day the computer was installed,
when its file system keeps that day, and are left out when it does not.

It is worked out from what is on the computer when you open it, shown, and gone when you close it: nothing is collected,
kept or sent, and a line it cannot read is left out. **Not this year** puts the card away until next year.

```bash
ame desktop your-year now    # the page in a terminal, at any time
ame desktop your-year off    # no card
```

## The weather

Off until you choose it, because it sends your place to weather services each time the weather is shown:

- the place you chose in Weather to MET Norway (api.met.no) and OpenWeatherMap (api.openweathermap.org, which refuses
  it, since Fedora's weather library has no key for it);
- the code of the nearest airport to NOAA's aviation reports (aviationweather.gov).

Its place in the calendar menu says so, with **Turn on…**, which asks once more; `ame desktop weather on` says what it
sends and turns it on. Then the temperature and sky show beside the clock and in the calendar menu, refreshed when you
open the menu, when the computer wakes and once an hour. If Weather has no place yet, the menu offers to open it.
`ame desktop weather off` stops it, and **What this machine sends** in Security lists it.

On or off, GNOME Shell itself asks for the weather once as you log in, and when Weather's place changes while the
screen is locked, if you set a place in Weather.

## Rest reminders

GNOME can remind you to look away from the screen now and then, and to stand up and move. The reminders are GNOME's own
notifications, with **Delay**, **Skip** and **Take**, and the screen fades gently when a break begins. Both are off
until you turn them on, here or in Control's **Rest reminders**; the times are in Settings, under **Wellbeing**.

```bash
ame desktop breaks on          # look away for 20 seconds every 20 minutes
ame desktop breaks movement    # stand up for 5 minutes every 30 minutes
ame desktop breaks both        # or off
```

## A resting screen

A computer left alone for a few minutes rests: a large clock and the day over your slowly drifting wallpaper, what is
playing, and when the screen will lock. The windows are hidden under it. Any key, a click or moving the pointer brings
everything back, and that key never reaches a window.

![A resting screen](media/resting-screen.webm)

- Off until you turn it on: Control (**Desktop**, **Resting**) or `ame desktop resting-screen on`.
- The screen locks when it would have anyway: the resting screen neither holds it back nor brings it forward.
- It does not come while something keeps the computer awake (a video, a presentation), a window is fullscreen, or the
  screen is shared or recorded. On battery, in the power saver and with reduced motion the wallpaper holds still.

```bash
ame desktop resting-screen after 5    # minutes left alone first, 1 to 30
```
