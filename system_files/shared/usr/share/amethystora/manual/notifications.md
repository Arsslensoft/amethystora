# Notifications

Notifications wait in the calendar's list (click the clock), where GNOME folds an app's into one stack that says how
many it holds after the app's name. Amethystora adds ways to hold banners back without losing anything.

## Do Not Disturb

GNOME's own switch, at the foot of the calendar's list. While it is on, banners do not pop up, and notifications still
wait in the list. Quiet, the digest, Later and mute never touch it: it stays yours.

## Privacy dots

While something uses the microphone, the camera, the screen or your location, a small pill in the top bar holds a dot
for each, always in this order: amber for the microphone, green for the camera, red for the screen (recorded or
shared), violet for the location.

![Privacy dots](media/privacy-dots.webm)

Click it for the list of all four, each with its own icon so that colour is never the only sign: the apps recording
sound, the apps using the camera (PipeWire says which), the screen with a **Stop** button, and the location. GNOME does
not say which app asks for your location, so the list only says that it is in use. While the dots are on, GNOME's own
icons for the same four are hidden.

**The recording glow.** While the screen is recorded or shared, a thin red glow runs round its edges, over everything,
so nobody forgets. It shows in a recording of the whole screen: the desktop cannot leave it out.

```bash
ame desktop pill dots on|off
ame desktop pill glow on|off
```

## Quiet

While a window is fullscreen on the main screen, or the screen is shared or recorded, banners are quiet: a
notification still arrives, waits in the list and counts on the clock's dot and on its app in the dock, but does not
pop up or make a sound. Calls, alarms, timers and anything marked urgent still come through.

![Quiet](media/notification-quiet.webm)

When quiet ends, what came meanwhile is not shown one after another: the pill says **2 waiting in the list**, and a
click opens the list. While the top bar shows, the pill says that it is quiet and how many are held.

**Quiet at night** does the same between Night Light's times, whether or not Night Light itself is on: the times you
set in Settings, or sunset to sunrise. It is off until you turn it on.

```bash
ame desktop quiet on|off
ame desktop quiet night on|off
```

## The digest

With the digest on, banners that are not urgent wait all day and come together as one card at the digest times (12:00
and 18:00 to begin with): one row per app, **Show all** (the list) and **Clear**. Nothing is lost when the card goes:
everything is in the list. Control offers four sets of times.

![The digest](media/notification-digest.webm)

```bash
ame desktop digest on|off
ame desktop digest times 12:00 18:00     # other times, up to six
```

### Apps that come through or always wait

- **Always come through**: while the digest is on, these apps' banners show at once (quiet, a mute or a focus profile
  still hold them).
- **Always wait**: these apps' banners wait for the next digest time even while the digest itself is off, so a noisy
  chat or store can wait while everything else comes as it comes.

An app is on one list at most, and calls, alarms and urgent banners always pass. Pick them in Control (**Desktop**,
**Notifications**) or in a terminal:

```bash
ame desktop digest through org.gnome.Calendar.desktop     # or none, to pick from a list
ame desktop digest wait org.gnome.Fractal.desktop
ame desktop digest wait reset
```

## Later and mute

Rest the pointer on a banner (or reach it with the keyboard): after the app's own buttons is **Later**, with **In an
hour**, **This evening** (18:00), **Tomorrow morning** (09:00) and **When I next unlock**. The banner goes, the pill says
when it comes back, with **Undo**, and at that time it comes back as it was, with its own buttons, and rings once. With
a banner focused, `L` is Later in an hour.

![Later](media/notification-later.webm)

**Mute Notes for an hour** (with the app's own name) holds that app's banners for an hour, as quiet does: they still
arrive in the list. Calls, alarms and urgent ones are never muted, only snoozed by your own hand.

### Nothing waits out of sight

- At the top of the calendar's list, **Later** says when each comes back, with **Now**, and **Muted** says until when
  and how many it holds, with **Unmute**, which shows the held ones together.
- A snoozed notification also stays in the list, so its buttons work from there at any time.
- What waits is in memory only, never on disk: it lasts through the lock screen, and logging out forgets it, as GNOME
  forgets the notifications themselves.

```bash
ame desktop notifications later on|off
ame desktop notifications later-times 19:00 08:00    # other evening and morning times; Control offers three pairs
```

## Volume and brightness

The volume, the brightness and the other signs GNOME shows for a key (Caps Lock, flight mode) appear as a slim pill
just under the top bar, instead of a card low in the middle of the screen.
