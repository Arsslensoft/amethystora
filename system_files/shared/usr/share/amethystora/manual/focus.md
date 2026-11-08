# Focus and presenting

Focus profiles quieten the desktop for work, games or sleep, and Present mode keeps the screen awake and calm for a
call, a class or a demo. Both lay themselves over your settings and give everything back when they end.

## Focus

There are three profiles: **Work**, **Games** and **Sleep**. Pick one in the **Focus** tile of the quick settings (the
chips Off, Work, Games and Sleep), in Control, or in a terminal. The tile says what the profile does and when it ends:
when you turn it off, after **1 hour**, or **tomorrow** (at six in the morning). While one is on, the live pill shows
its mark and the time, and a click on it has **Turn off**.

```bash
ame desktop focus work          # or games, sleep
ame desktop focus work hour     # or tomorrow
ame desktop focus off
ame desktop focus               # what is on
```

### What each profile does

As the image ships them:

- **Work**: banners only from Calendar, Clocks and calls, without a sound; the rest waits in the list.
- **Games**: banners held, the dock hides while a window touches it, the performance power mode, and Night Light
  paused.
- **Sleep**: banners held, Night Light on at its warmest, Daylight's night picture over your wallpaper, power saver
  and an amber accent.

Calls, alarms and anything urgent always come through. Held banners wait in the list and count on the clock's dot,
and when the profile ends the pill says how many are waiting (with the [digest](notifications.md#the-digest) on, they
wait for the digest). Do Not Disturb is never touched.

### What it changes, and gives back

Nearly everything a profile does is held in memory only: the dock's, the accent's, the wallpaper's and the banners'
settings are never written, and the power mode is a hold the power daemon lets go of when the profile ends, or if the
desktop stops. A setting you changed yourself meanwhile stays as you set it.

- **Sleep's Night Light** is the one exception: it turns Night Light on as warm as it goes, and puts both back when
  Sleep ends, at the next log-in if the desktop stopped first. Night Light's times are never changed, so outside them
  Sleep's warmth begins when they do.
- **Work** leaves the power mode as you set it: the power daemon can hold only performance or power saver.
- **The lock screen** runs none of this: while it is locked the profile's power mode is let go, and taken again when
  you unlock; Night Light stays as the profile set it.

### Changing a profile

Each line of each profile can be changed in Control, in **Focus: Work**, **Focus: Games** and **Focus: Sleep**: which
banners come through and from which apps, notification sounds, the dock, solid glass and shorter motion (what power
saver does, for games played in a window), Night Light, the power mode, the accent, the wallpaper and the screen in
grey (one more pass over the whole screen every frame, so it costs some battery). Once you have
[saved sets](workspaces.md) of windows, a profile can offer to open one when you pick it yourself; it always asks
first.

```bash
ame desktop focus edit work power performance
ame desktop focus edit work apps     # choose the apps
ame desktop focus edit work reset    # the image's own
```

### Starting by itself

Two triggers, both off until you turn them on in Control or with `ame desktop focus trigger games on` and
`ame desktop focus trigger sleep on`:

- **Games while a game is in front** starts Games when a game gets the keyboard and ends it when that game closes.
  Steam, Lutris and the other launchers are not games themselves; a game Steam starts is.
- **Sleep at Night Light's times** starts Sleep at Night Light's evening time and ends it at its morning time.

A profile you pick wins over both. Turning Focus off while a trigger has it on keeps it off until that trigger's next
start: the next evening, or the next game.

## Presenting

Present mode is off until you turn it on in Control (**Desktop**, **Presenting**) or with `ame desktop present on`.
Then **Present** is a tile in quick settings, and while it is on:

- the screen stays awake: it does not dim, lock or sleep by itself (you can still lock it yourself);
- banners wait in the list, as [Quiet](notifications.md#quiet) holds them; calls, alarms and urgent ones still come
  through, and when you stop, the top bar says how many are waiting;
- the top bar says **Presenting** and for how long; click it to stop;
- the pointer is easy to follow: a **Halo** round it (the default), or a **Torch** that dims everything but a circle
  round it. `ame desktop present mark halo`, `torch` or `none` changes it.

With [the phone as a slide clicker](phone-calls.md#the-phone-as-a-slide-clicker) on, a paired phone goes to the next or
previous slide, or blanks the screen, while you present.

### A screen or projector plugged in

Amethystora asks whether to present on it, once for each screen; nothing happens until you click **Present**, and
presenting then ends when that screen is unplugged. Started while you share your screen in a call, it ends with the
share. `ame desktop present ask off` stops the question.

### Drawing on the screen

`Super+Alt+D` lets you draw over everything while Present mode is on, presenting or not: a pen, a highlighter and three
colours, in a bar at the bottom (a pen's pressure is not read). The windows under the ink take no clicks or keys until
you finish.

| Key | Does |
| --- | --- |
| `Ctrl+Z` | Take the last stroke back |
| `1`, `2`, `3` | Pick a colour |
| `P` / `H` | The pen / the highlighter |
| `Esc` (or **Clear**) | Wipe the ink and give the windows back |

Nothing is ever saved; to keep it, press `Print` while you draw and take a screenshot with the ink.

## Night Light and the screen's colour

Night Light makes the screen's colours warmer, more yellow, from sunset to sunrise or between the times you choose,
which is easier on the eyes at night. If the screen turns yellow in the evening, that is Night Light. Turn it on or off
in the quick settings at the top right, and set its times and warmth in **Settings**, **Displays**, **Night Light**.
[Quiet at night](notifications.md#quiet) follows the same times.
