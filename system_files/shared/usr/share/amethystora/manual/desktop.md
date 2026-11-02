# Getting around

The desktop is GNOME, arranged for the keyboard. Everything here also works with the mouse, but once
the keys are in your fingers you will rarely reach for it.

## The top bar

- **On the left**, the Amethystora logo, a menu of the system's tools: about this computer, this
  manual, Settings, Bazaar, Extensions, Mission Center, the terminal and DistroShelf. A
  middle click on it opens the overview. Then the six workspaces: the one you are on is
  highlighted; click another to go there, or the one you are on to show or hide the overview. An empty
  one opens the overview, ready to start something. Scroll the mouse wheel, or two fingers on a
  touchpad, anywhere on the bar to go to the next or previous workspace. Drag a workspace along the row to move it, its windows with it, or
  drop a window from the overview on one to send it there. Right-click a workspace to give it a name
  (until you do, it is named after its main app).
- **In the middle**, the clock, with [the weather](#the-weather) beside it once you turn it on. Click it,
  or press `Super+V`, for notifications, the calendar and [Today](#today). While something is happening,
  the clock becomes the [live pill](#the-live-pill).
- **On the right**, CPU, memory and network meters, the icons apps keep in the tray, then the system
  menu: Wi-Fi, sound, power and the rest of the [quick settings](#quick-settings). Right-click a tray icon for its app's
  menu, double-click it to open the app, and middle-click it for the app's second action. A Flatpak app
  shows its icon only when it may talk to the tray (`org.kde.StatusNotifierWatcher`), and an old X11
  one only while X11 apps are allowed. Click a meter for the details: each processor, the memory and the
  network over the last few minutes, the busiest processes, and a button that opens Mission Center.
  The network meter counts your Wi-Fi and wired connections only, so a VPN or a container's traffic
  is not counted twice.

The meters and the workspaces follow the colours of the current [theme](themes.md). The bar can
also show [widgets](widgets.md) of your own, which the AI agent makes for you once you turn it on.

A fullscreen video or game keeps the whole screen; push the pointer against the top edge and the bar
comes down over it, and goes when you move away. `ame desktop top-bar peek off` turns that off. With
two monitors or more, the bar and the dock are on the main one until you ask for them on the others: see
[Several monitors](#several-monitors).

### Arranging the bar

Control's **Desktop** page draws the bar as four places, **Left**, **Centre**, **Right** and **Hidden**. Drag an
item to another place, or along its own, to move it there; each item also has a menu that does the same from the
keyboard. The clock can go to the left, the workspaces to the middle, the tray anywhere. The clock and the system
menu can move but never hide, and the privacy dots always stay beside the system menu. **As the image has it** puts
everything back. In a terminal:

```bash
ame desktop top-bar layout show            # each item, and where it is
ame desktop top-bar item clock left 0      # the clock first on the left
ame desktop top-bar item network hidden    # the network meter off the bar
ame desktop top-bar layout reset           # as the image has it
```

Each meter has a switch of its own in Control, under **Top bar**, and in a terminal:

```bash
ame desktop top-bar meter processor off    # or memory, network; on brings it back
```

A meter that is off or hidden reads nothing, and its menu is gone with it, **Network by app** included. The
[busy app](#power-you-can-see) chip has its own switch and keeps reading the processor until you turn that off too.

The items are `logo`, `workspaces`, `tiling`, `clock`, `processor`, `memory`, `network`, `tray`,
`clipboard`, `phone`, `shield`, `busy`, `keyboard`, `a11y`, `system`, and `widget-NAME` for each of your
[widgets](widgets.md). Something another extension puts on the bar keeps its own place.

### The bar's size

Compact (28 pixels), regular (32, the image's) or roomy (40): the bar and everything on it grow or shrink together,
the icons, the workspaces and the clock's pill. A small laptop screen gets room back; a large screen across the room
gets bigger targets. [Touch mode](#touch-mode) still makes it taller than any of them.

```bash
ame desktop top-bar size compact
```

### At the bottom, or hidden

The bar can sit at the bottom of the screen instead (`ame desktop top-bar position bottom`): its menus open upwards,
and a dock at the bottom sits just above it. It can also hide until you want it (`ame desktop top-bar autohide on`):
the windows get its room, and pushing the pointer against its edge brings it in, as over a fullscreen window. Both are
off until you choose them, and both are in Control's **Top bar** group. Notifications still appear at the top.

### The clock

What the clock shows is GNOME's own choice, in Control's **Clock** group or with `ame desktop clock`: 24 or 12
hours, seconds, the weekday and the date, and week numbers in the calendar. A second time zone, for family or
colleagues abroad, shows after the clock with its city:

```bash
ame desktop clock format 12h
ame desktop clock seconds on       # the clock then changes every second
ame desktop clock week-numbers on
ame desktop clock zone Asia/Tokyo  # "Tokyo 06:41" after the clock; off takes it away
```

A zone is a name from the computer's own time-zone data, an area and a city such as `Europe/Paris` or
`America/New_York`; Control offers a city in each part of the world.

## Quick settings

The menu at the right end of the top bar opens on tiles, two to a row, each saying what it knows: the
Wi-Fi network and how strong it is, the earbuds and their battery, the power mode, and, while a paired
phone is connected, the phone and its battery (click it for the Phone app). Click a tile to switch it, and
its arrow for its menu. Under the tiles are the sliders, then the apps playing sound, a swatch for every
theme, and Screenshot, Lock, Log out, Settings and Power.

- **Moving and hiding tiles.** Hold a tile for half a second (or press `Menu` on it): the tiles shrink a
  little and each gets a round button at its corner. Drag a tile to its new place; the button puts it
  away, into a Hidden row, where the same button brings it back. With the keyboard, `Ctrl` and an arrow
  moves the tile you are on, `Delete` hides it and `Enter` brings a hidden one back. Done, `Esc` or
  closing the menu ends it. `ame desktop quick-settings reset` puts every tile back in its first place
  and shows them all, and so does **Quick settings tiles** in [Control](control.md).
- **The battery.** On a laptop the first tile is the battery: how full it is and when it runs out or is
  full. Click it for **Keep the battery at 80%**, where the laptop can stop charging early so the battery
  lasts more years; the switch is GNOME's own charge limit, which UPower keeps across restarts. A laptop
  that cannot stop charging early has no such switch.
- **Each app's volume.** While apps play sound, each has its own slider and mute button under the
  volume slider, and the buttons beside "Playing now" send the sound to another output, as the arrow of
  the volume slider does.
- **The theme.** A tap on a swatch switches to that theme, as `ame desktop theme` does: the desktop and
  Amethystora's apps change at once, other apps the next time they start.
  `ame desktop quick-settings theme off` hides the swatches, and `on` brings them back; you can also
  hide them as you hide a tile.

## Power you can see

Why is the fan loud, and why did the battery go so fast? The battery tile's menu, on a laptop, opens on
the answer: how full the battery is and when it will be empty or full, the watts it gives or takes now
and when it was unplugged, the apps that used the most since it was last plugged in or out, and the
battery's health (how much it holds against when it was new, and how many times it has been charged,
when the battery says). The apps are worked out from the processor time each one used, so they are an
estimate, and the menu says so: the screen and the graphics are not counted, nor an app that came and
went in between. Nothing is measured while the menu is closed but one look when you log in and each time
you plug in or unplug.

**A busy app.** When the processor has been at 70% or more for two minutes, a chip beside the meters in
the top bar names the app that kept it busy, such as **Firefox is busy**. Click it for **Show it**,
**Not now** and **Quit Firefox**, which asks the app to quit as its own menu would. It is said once per
app per hour, never while a window is fullscreen in front (a game), and the chip goes when the
processor calms down. It rides the meters' own reading, so it still reads the processor with its meter off the bar.

**Your devices' batteries.** The same menu lists the battery of every device that says how full it is: a wireless
mouse or keyboard, headphones and headsets, a game controller, a pen, and your paired [phone](hardware.md#phones),
under **This computer**. A row says **charge it soon** under 20%, and a notice says once when a device is at 15% or less
(again only after it has been charged). On a computer without a battery of its own, the tile shows as **Batteries**
while such a device is there, naming the emptiest. It is what UPower already knows, followed as UPower says it changes:
nothing is asked on a timer.

`ame desktop power battery on|off`, `ame desktop power busy on|off` and `ame desktop power devices on|off`, also in
[Control](control.md).

## Today

The calendar menu, under the calendar, holds the day: the events from your calendars, the tasks due
today and the overdue ones from [Notes](notes.md#tasks), your world clocks and the weather. The tasks
are there while Notes is open and unlocked, or not encrypted, and only on today's page of the calendar;
click them to open Notes' tasks. Nothing is asked of Notes until you open the menu, and Notes tells the
calendar nothing but the titles of those tasks, and nothing at all when the menu is closed. Notes answers
GNOME Shell alone: an app in a sandbox cannot ask, but a program you run yourself outside one could, as it
could read anything else of yours while your notes are open. `ame desktop today tasks off` leaves them out.

### Getting set

For your first four weeks, the calendar menu starts with **Getting set**: the few steps that protect
the machine and make it yours. Pick a look, automatic updates, a backup of your home folder, a PIN to
unlock the disk, and pairing your phone. Each is ticked by what the machine already says (a backup
ran, a phone is paired, the security report says the disk asks for its PIN), never by clicking it; a
click opens the place that does it, and the security line only opens [Security](security.md). A step
the machine has nothing to do about (a disk that is not encrypted, say) is left out. Nothing is read
until you open the menu. **Hide** puts it away for good, as does `ame desktop getting-set off`; once
every step is done it goes by itself.

## The weather

The weather is off until you choose it, because it sends your place to weather services: the place you
chose in Weather goes to MET Norway (api.met.no) and OpenWeatherMap (api.openweathermap.org, which
refuses it, since Fedora's weather library has no key for it), and the code of the nearest airport to
NOAA's aviation reports (aviationweather.gov), each time the weather is shown. Its place in the calendar
menu says so, with **Turn on…**, which asks once more before it turns it on; `ame desktop weather on`
says what it sends and turns it on. Once it is on, the temperature and the sky show beside the clock and
in the calendar menu, refreshed when you open the menu, when the computer wakes and once an hour. If
Weather has no place yet, the menu offers to open it. `ame desktop weather off` stops it, and **What this
machine sends** in Security lists it. Whether it is on or off, GNOME Shell itself asks for the weather
once as you log in, and when Weather's place changes while the screen is locked, if you set a place in
Weather.

## Rest reminders

GNOME can remind you to look away from the screen now and then, and to stand up and move. Both are off
until you turn them on: `ame desktop breaks on` (look away for 20 seconds every 20 minutes),
`movement` (stand up for 5 minutes every 30 minutes), `both`, or `off`, or Control's **Rest reminders**.
The reminders are GNOME's own notifications, with **Delay**, **Skip** and **Take**, and the screen fades
gently when a break begins. The times are in Settings, under **Wellbeing**.

## The live pill

While something is happening, the clock in the middle of the top bar shows it, on a small round pill.
When nothing is, it is the clock again, exactly as before. Only one thing shows at a time, the first of
these that is happening: a call ringing on your phone, a recording of the screen, a link from your phone,
a meeting about to start, the battery's question at 20%, a word that something just finished, a timer, what is playing, an update
ready, and quiet. Click the pill for its card, with its buttons and, below them, whatever else is
happening; the card's last row is the date and time, which opens the calendar. A right-click on the
pill, or `Super+V`, always opens the calendar.

- **A timer.** At the end of the calendar's column, under **Start a timer**, are 1, 5, 10 and 25 minutes.
  The pill counts it down; its card adds a minute, pauses or cancels it. While one runs, the same
  buttons add that many minutes. When it ends you hear it, a notification says so (it comes through
  even when banners are quiet), and the pill says **Timer done**. A timer runs on while the screen is
  locked, and rings as soon as you unlock it if it ended meanwhile.
- **What is playing**, from any player that tells the desktop (as music and video apps do): the cover,
  the title and the artist, and a click on the small pause button pauses it. The card has Back, Pause
  and Next, and how far into the track you are. A player you paused stays for ten minutes.
- **A call** on your [paired phone](phone.md): who is calling, and **Mute ringer**. The card also has
  **Reply with a text**. You answer on the phone itself.
- **A recording** of the screen: how long it has run, and **Stop**. GNOME's own recordings go to
  Videos › Screencasts.
- **A link from your phone**: the address, with **Open** and **Not now**. A link is shown, never opened
  by itself: **Open in browser** opens it, **Copy** copies it, and **Not now** leaves it in the
  notification list. A name in another alphabet is shown in its plain form (`xn--…`), so a look-alike
  cannot pass for the site it imitates.
- **An update ready**: **Ready · quick restart** when the update can finish without restarting the
  computer, else **Ready · restart**. **Restart now** opens [System Updates](updates.md#updating-now),
  which asks before it restarts anything; **Tonight** puts it out of sight until 8 in the evening.
- **Updated**, once, the first time you log in after an update: what it brought, and
  [What's new](updates.md#whats-new) in the Manual.
- **A meeting about to start**: see [Meetings in the pill](#meetings-in-the-pill).
- **The battery.** Rest the pointer on the battery in the top bar for the time left, or the time until
  it is full. At 20%, the pill turns amber and offers power saving, once: it never turns it on by itself.

`ame desktop pill off` gives you GNOME's clock alone; `ame desktop pill on` brings the pill back.

### Meetings in the pill

Five minutes before an event of today's calendar that has a video-call link, the pill shows it with a
countdown, *Design review in 4 min*, and then *Design review now* for ten minutes or until it ends. Its card
has the times, the calendar it is in, the link, **Later** and **Join**. **Join** opens the link in your
browser; nothing opens until you click it. **Later** puts it away until the meeting starts, and for good once
it has.

The events are the ones the calendar menu shows, from the calendars you added in Settings › Online Accounts
or in Calendar. The link is read from the event itself, as the calendar keeps it: its conference link, its
location, its link or its description, and only a link that looks like a call counts. Nothing is sent
anywhere. `ame desktop pill meetings off` turns it off, and `on` back on; Control has it as **Meetings in
the pill**.

## Privacy dots

While something uses the microphone, the camera, the screen or your location, a small pill in the top bar
holds a dot for each, always in this order: amber for the microphone, green for the camera, red for the
screen (recorded or shared), violet for the location. Click it for the list of all four, each with its
own icon so that colour is never the only sign: the apps recording sound, the apps using the camera
(PipeWire says which), the screen with a **Stop** button, and the location. GNOME does not say which app
asks for your location, so the list only says that it is in use. While the dots are on, GNOME's own icons
for the same four are hidden.

**The recording glow.** While the screen is recorded or shared, a thin red glow runs round its edges, over
everything, so nobody forgets. It shows in a recording of the whole screen: the desktop cannot leave it
out.

`ame desktop pill dots on|off` and `ame desktop pill glow on|off` turn each off and on again.

## Privacy

### Share guard

While your whole screen is shared, in a video call or a remote desktop, the windows of Notes, Security,
Backups and the password managers (KeePassXC, Bitwarden, Secrets, Passwords and Keys, Proton Pass, 1Password,
Authenticator), and Files windows showing your [private folder](security.md#a-private-folder-that-locks-with-the-screen),
are covered: blurred so that no word can be read, and labelled **Hidden while you share**. A window that opens
during the share is covered before it is first drawn, and the live pill says once what is covered.

The desktop cannot leave a window out of a picture of the whole screen, so the cover is on your screen
too. To show one window for the rest of the share, hold a click on its cover for a moment: it stays
uncovered, with its menus and dialogs, until the share ends. While someone else controls your computer
through the share (a remote desktop), holding does nothing, since the hold could be theirs. The covers
come at once when a share starts and go as soon as the desktop knows you share single windows only;
nothing is covered while you only record the screen yourself.

- `ame desktop share-guard blur|card|off`: blurred (the default), a solid card, or nothing covered.
- `ame desktop share-guard apps` picks the apps covered, in a terminal; `ame desktop share-guard apps
  APP.desktop...` names them by their launcher, and `apps reset` puts back the list above.
- `ame desktop share-guard private on|off`: the private folder's windows, covered or not.

A cover keeps a window's contents from being shown by accident; it is not a lock. A notification, the
window's title in the top bar and the dock, and any app the list does not name are shown as ever. An app
that hides passwords by itself is safer still.

### The shield

A tile in quick settings, **Shield**, says how the machine's protection stands: *Protected*, or how many
things need a look, and whether the network you are on is home or away. Its menu has the security report's
result (for administrators, who may read the report the security watcher writes; others see the rest of
the menu), the network and its [firewall zone](security.md#on-from-the-start), [encrypted
DNS](security.md#encrypted-dns), [USB protection](security.md#block-usb-devices-you-did-not-plug-in), the
private folder, open or closed, and whether apps may ask for the camera and the microphone. It changes
nothing: every line opens [Security](security.md) on its page, where every switch is.

`ame desktop shield tile|mark|attention|off` puts it elsewhere: a shield in the top bar beside the
system's icons, green, amber when something needs a look; the same shield only while something needs a
look; or nothing. It reads what it shows when you open it, and costs nothing otherwise.

### Network by app

Off until you turn it on (`ame desktop network-apps on`, or Control): the network meter's menu then ends
with the apps that have a connection open, how many, and to where. A place shows as a name only when the
machine has a name for its address already (your hosts file, your own network's names, a reverse name it
looked up before); most places on the internet therefore show as addresses. Nothing is asked anywhere to
find a name, and nothing leaves the machine. Only your own programs are
listed, and only while the menu is open. How much each app sends would need an administrator's rights, so
the list counts connections, not bytes. **Network per app, in Security** opens the Apps page, where an
app's network can be taken away.

### A new network's zone

The first time you join a network, a notification names the firewall zone it is in and what that means: in the
default zone, phone pairing is closed and nothing on this computer is shared; in the home zone, phones can pair;
in the away zone, nothing that starts on that network reaches the computer. **Open Security** opens its Network
settings, where a network can be named home ([home networks](security.md#on-from-the-start)). It changes
nothing itself, and it says this once per network: the networks this account has joined before are remembered
by NetworkManager's id for them, never by name. `ame desktop zone-notice off` turns it off.

## Notifications

GNOME folds an app's notifications in the calendar's list into one stack; a folded stack shows how many it
holds after the app's name.

**Do Not Disturb** is GNOME's own switch, at the foot of that list: click the clock to open it. While it
is on, banners do not pop up, and notifications still wait in the list.

### Quiet

While a window is fullscreen on the main screen, or the screen is shared or recorded, banners are quiet:
a notification still arrives, waits in the list and counts on the clock's dot and on its app in the dock,
but does not pop up or make a sound. Calls, alarms, timers and anything marked urgent still come through.
When quiet ends, what came meanwhile is not shown one after another: the pill says **2 waiting in the
list**, and a click on it opens the list. While the top bar shows, the pill says that it is quiet and how
many are held.

**Quiet at night** does the same between Night Light's times, whether or not Night Light itself is on:
the times you set for it in Settings, or from sunset to sunrise. It is off until you turn it on.

`ame desktop quiet on|off` and `ame desktop quiet night on|off`. Quiet never touches Do Not Disturb, which
stays yours, in the calendar.

### The digest

With the digest on, banners that are not urgent wait all day and come together, as one card, at the
digest times (12:00 and 18:00 to begin with): one row for each app, **Show all** (the list) and **Clear**.
Nothing is lost when the card goes: everything is in the list. `ame desktop digest on|off`, and
`ame desktop digest times 12:00 18:00` for other times (up to six). Control offers four sets.

Two lists change that for the apps you pick. **Always come through**: while the digest is on, these apps' banners show
at once, as they would without it (quiet, a mute or a focus profile still hold them). **Always wait**: these apps'
banners always wait for the next digest time, even while the digest itself is off, so a noisy chat or store can wait
while everything else comes as it comes. An app is on one list at most; calls, alarms and urgent banners always pass.
Pick them in Control, **Desktop**, **Notifications**, or in a terminal:

    ame desktop digest through org.gnome.Calendar.desktop     # or none, to pick from a list
    ame desktop digest wait org.gnome.Fractal.desktop
    ame desktop digest wait reset

### Later and mute

A reminder you cannot act on now can come back when you can. Rest the pointer on a banner (or reach it
with the keyboard) and, after the app's own buttons, there is **Later**: it offers **In an hour**,
**This evening** (until 18:00), **Tomorrow morning** (09:00) and **When I next unlock**. The banner goes,
the pill says when it comes back with an **Undo**, and at that time it comes back as it was, with its own
buttons, and rings once. With a banner focused, `L` is Later in an hour. **Mute Notes for an hour** (with
the app's own name) holds that app's banners for an hour, as quiet does: they still arrive in the list.
Calls, alarms and urgent ones are never muted, only snoozed by your own hand.

Nothing waits out of sight: at the top of the calendar's list, **Later** says when each comes back, with
**Now**, and **Muted** says until when and how many it holds, with **Unmute**, which shows the held ones
together. A snoozed notification also stays in the list itself, so its buttons work from there at any
time. What waits is kept in memory only, never written to disk: it lasts through the lock screen, and
logging out forgets it, as GNOME forgets the notifications themselves.

`ame desktop notifications later on|off`, and `ame desktop notifications later-times 19:00 08:00` for
other evening and morning times; Control offers three pairs.

### Volume and brightness

The volume, the brightness and the other signs GNOME shows for a key (Caps Lock, flight mode) appear as
a slim pill just under the top bar, instead of a card low in the middle of the screen.

## Focus

A focus profile lays itself over your settings while it is on, and when it ends everything is as you left
it. There are three: **Work**, **Games** and **Sleep**. Pick one in the **Focus** tile of the quick settings
(the four chips, Off, Work, Games and Sleep), in Control or with `ame desktop focus work` (or `games`,
`sleep`). The tile says what the profile does and when it ends: when you turn it off, after **1 hour** or
**tomorrow** (at six in the morning); `ame desktop focus work hour` and `ame desktop focus work tomorrow` do
the same. While a profile is on, the live pill shows its mark and the time, and a click on it has **Turn
off**. `ame desktop focus off` ends it, and `ame desktop focus` says what is on.

As the image ships them:

- **Work**: banners only from Calendar, Clocks and calls, without a sound; the rest waits in the list.
- **Games**: banners held, the dock hides while a window touches it, the performance power mode, and
  Night Light paused.
- **Sleep**: banners held, Night Light on at its warmest, Daylight's night picture over your wallpaper,
  power saver and an amber accent.

Calls, alarms and anything urgent always come through. Banners a profile held wait in the list, count on
the clock's dot, and when it ends the pill says how many are waiting (with the [digest](#the-digest) on,
they wait for the digest). Do Not Disturb is never touched. Nearly everything a profile does is held in
memory only: the dock's, the accent's, the wallpaper's and the banners' settings are never written, and
the power mode is a hold the power daemon lets go of when the profile ends, or if the desktop stops. The
one exception is Sleep's Night Light: it turns Night Light on and makes it as warm as it goes, and puts
both back as they were when Sleep ends, at the next log-in if the desktop stopped first. A setting you
changed yourself meanwhile stays as you set it. Night Light's times are never changed, so outside them
Sleep's warmth begins when they do. Work leaves the power mode as you set it: the power daemon can hold
only performance or power saver. The lock screen runs none of this, so while the screen is locked the
profile's power mode is let go, and taken again when you unlock; Night Light stays as the profile set it.

Each line of each profile can be changed in Control, in the groups **Focus: Work**, **Focus: Games** and
**Focus: Sleep**: which banners come through and from which apps, notification sounds, the dock, solid
glass and shorter motion (what power saver does, for games played in a window), Night Light, the power
mode, the accent, the wallpaper and the screen in grey (one more pass over the whole screen for every
frame, so it costs some battery). Once you have [saved sets](#workspaces) of windows, a profile can offer
to open one when you pick it yourself; it always asks first. From a terminal:
`ame desktop focus edit work power performance`, `ame desktop focus edit work apps` to choose the apps,
and `ame desktop focus edit work reset` for the image's own.

Two triggers turn a profile on by themselves, both off until you turn them on in Control or with
`ame desktop focus trigger games on` and `ame desktop focus trigger sleep on`: **Games while a game is in
front** starts Games when a game gets the keyboard and ends it when that game closes (Steam, Lutris and
the other launchers are not games themselves; a game Steam starts is), and **Sleep at
Night Light's times** starts Sleep at Night Light's evening time and ends it at its morning time. A
profile you pick wins over both. Turning Focus off while a trigger has it on keeps it off until that
trigger's next start: the next evening, or the next game.

## Presenting

Present mode is for a call, a class or a demo. It is off until you turn it on in Control (**Desktop**,
**Presenting**) or with `ame desktop present on`. Then **Present** is a tile in quick settings, and while it is
on:

- the screen stays awake: it does not dim, lock or go to sleep by itself (you can still lock it yourself);
- banners wait in the notification list, as [Quiet](#quiet) holds them; calls, alarms and urgent ones still
  come through, and when you stop, the top bar says how many are waiting;
- the top bar says **Presenting** and for how long; click it to stop;
- the pointer is easy to follow: a **Halo** round it (the default), or a **Torch** that dims everything but a
  circle round it. `ame desktop present mark halo`, `torch` or `none` changes it.

With [the phone as a slide clicker](phone.md#the-phone-as-a-slide-clicker) on, a paired phone goes to the next or
previous slide, or blanks the screen, while you present.

When you plug in a screen, a projector for instance, Amethystora asks whether to present on it, once for each
screen; nothing happens until you click **Present**, and presenting then ends when that screen is unplugged.
Started while you share your screen in a call, it ends with the share. `ame desktop present ask off` stops the
question.

### Drawing on the screen

`Super+Alt+D` lets you draw over everything on the screen while Present mode is on, presenting or not: a pen, a
highlighter and three colours, in a bar at the bottom (a pen's pressure is not read). The windows
under the ink take no clicks or keys until you finish. `Ctrl+Z` takes the last stroke back, `1`, `2` and `3`
pick a colour, `P` and `H` the pen and the highlighter, and `Esc` (or **Clear**) wipes the ink and gives you the
windows back. Nothing is ever saved; to keep it, press `Print` while you draw and take a screenshot with the ink.

## Desks

A laptop lives at several desks. GNOME already remembers how the screens were arranged at each; a desk remembers the
rest: the sound output, where the dock is, a [Focus](#focus) profile and the power mode. Desks are off until you turn
them on in Control (**Desks**, on the Desktop page) or with `ame desktop desks on`.

A desk is known by its screens: their maker, model and serial number, as the screens report them. Two identical
screens without serial numbers are told apart by the socket they are plugged into, and two desks with the same
screens by their Thunderbolt docks; a Thunderbolt drive plugged in at a desk leaves it the same desk. The computer's
own screen does not count, so a desk is the same with the lid open or shut, and unlocking the screen at the same desk
is no arrival.

The first time the computer is at a set of screens, a card at the top right asks: it shows how the four things are
set now, and **Set up** remembers them for that desk (Focus only if a profile is on). **Not now** is remembered: that
desk does not ask again until `ame desktop desks ask`. Left unanswered, the card asks at the next arrival. After that, arriving at the
desk sets what differs at once, and a toast at the bottom says what it set, with **Undo**. When you unplug, whatever
the desk set and is still as it set it goes back to how it was before. Something you changed yourself meanwhile stays
as you changed it.

With **Learn from what I change here** on (it is, with Desks), picking another sound output or moving the dock at a
desk asks whether the desk should remember it, once for each change (a sound output that just appeared, like
headphones connecting, is not asked about). A desk's power mode is set only while no Focus profile or game holds
one, and a held mode is never taken for the desk's own. A desk can also offer one of your [saved sets](#saved-sets) of
windows: choose it in Control, or `ame desktop desks saved ID NAME`; arriving then asks before opening it. A desk
never changes a security setting and never starts an app by itself.

`ame desktop desks list` lists the desks with what each sets, `ame desktop desks rename ID Office desk` names one
(until then a desk is named after what it has, like "2 screens"), and `ame desktop desks forget ID` forgets it. The
desks are kept in `~/.config/amethystora/desks.json`, which only your account can read. The screens' serial numbers
stay there: they are never in [your setup file](setup.md), which carries only whether Desks are on.

## Sounds

The desktop's own sounds are Amethystora's **Soft** set: lower and rounder, for a quiet room. Three more sets play the
same events in other voices: **Glass**, bright, short chimes; **Wood**, warm knocks and taps; **Retro**, eight-bit
blips. Each is made from arithmetic by the image's own script, nothing recorded. You hear them for a
message from a chat app that asks for one (GNOME plays no sound for an ordinary notification), a text message from
your [phone](hardware.md#phones), plugging in the power, a screenshot, each step of the volume, an error and the
bell, and a slow chord once the desktop has started. Any other event an app asks for comes from the standard sound
theme.

`ame desktop sounds soft|glass|wood|retro` picks a set, and `ame desktop sounds off` makes the desktop silent: it
makes no sound of its own, and apps can still make theirs (a call, music, a game). Control has the same choice under
Appearance, in the **Sound** group. Choosing an alert sound in **Settings**, **Sound**, gives the bell that sound;
Control then shows none of the sets as chosen, and `ame desktop sounds soft` goes back to the whole Soft set.

### Pause when the headphones come off

When the headphones or the headset you are listening on are unplugged or go away (a Bluetooth headset out of range or
switched off), every player that is playing pauses, so that a podcast never bursts out of the laptop in a quiet room. A
notice says **Your headphones went away**, with **Play on speakers** to play on; when the headphones come back while it
shows, it offers **Play**. Picking the speakers yourself while the headphones stay plugged in pauses nothing. Players
are paused through their own media controls, the same ones the top bar's media card uses. It is on;
`ame desktop pause-on-unplug on|off`, also in Control.

### Clear voice

**Clear voice** takes background noise and echo out of your microphone: the fan, the keyboard, the street and the
room's echo. Calls hear your voice and little else, in every app, because the apps simply get a cleaner microphone:
PipeWire's own echo cancel and noise suppression (its webrtc processor), as an input of its own named **Clear voice**,
made the one in use while it is on. It works only while an app records, and costs some processor time then; nothing is
sent or saved. It is off; turn it on with the switch at the end of the microphone's menu in quick settings (shown
while an app records), with `ame desktop clear-voice on|off`, or in Control. Off puts back the microphone you had.

### Sound presets

**Sound presets** shape what an output plays, kept for each output: **Laptop** fills out thin laptop speakers,
**Speech** makes voices and podcasts clearer, **Bass** is for music, **Flat** is as it comes. Each is PipeWire's own
equaliser placed before that output, and only that output: headphones keep Flat until you pick otherwise. They are
off, since a preset changes every app's sound; `ame desktop sound-presets on` turns them on, and the output's menu in
quick settings (the arrow beside the volume) then shows the four with the curve of the one chosen. `ame desktop
sound-preset flat|laptop|speech|bass` sets the output in use, also in Control.

## The lock screen

The lock screen shows a large clock and the date, what is playing with its controls, and your notifications as one
row for each app, with how many it has: never what they say. To hide even those rows, turn off **Lock Screen
Notifications** in **Settings**, **Notifications**. The lock screen runs no extension, so it keeps Amethystora's own
colours whatever the theme. With **High Contrast** on (Settings, Accessibility), it is GNOME's own high-contrast lock
screen instead, with GNOME's clock and plain cards, which are easier to read.

## The first login

A new account's first login says hello in every language Amethystora speaks, the word written as if by hand in the gem's
light, your own language first, and then goes on to [Pick a look](welcome.md). It happens once. Any key or click goes
straight on. With reduced motion, or with animations off, the words stand still together and nothing is written. An
account that has logged in before is never greeted by itself. A dialog that opens over it, a password prompt for example, ends it.

```bash
ame desktop hello show      # see it again (not over a dialog that is open)
ame desktop hello off       # a new account of this setup is not greeted
```

Only a small mark that it has been shown is kept, `~/.local/state/amethystora/hello.json`.

## The reveal at login

When a session starts, the screen begins where the boot splash ended: its dark sky, and the gem in the middle. The gem
swells a little and flies into the top bar, where it becomes the Amethystora logo, while the desktop shows through the
sky and the top bar settles into place, all in about a second. It happens each time you log in, not when you unlock
the screen, and not at all with animations off (Settings, Accessibility). Turn it off on [Control](control.md)'s
**Appearance** page, or with `ame desktop transition reveal off`.

## Night Light and the screen's colour

Night Light makes the screen's colours warmer, more yellow, from sunset to sunrise or between the times you
choose, which is easier on the eyes at night. If the screen turns yellow in the evening, that is Night Light.
Turn it on or off in the quick settings at the top right, and set its times and how warm it is in
**Settings**, **Displays**, **Night Light**. [Quiet at night](#quiet) follows the same times.

## Finding things

![Search anything](media/super-space.webm)

- `Super+Space` opens a search box near the top of the screen, over your windows: type, and `Enter`
  opens the first result; `Ctrl+1` to `Ctrl+9` open the result with that number, which each of
  Amethystora's rows shows at its end. `Escape`, the same keys again or a click elsewhere closes it.
  Right-click a result for its menu, or drag an app onto the dock to pin it.
- `Super` alone opens the overview: every open window at once, with search at the top.
- `Super+A` shows every installed app: the ones you use most in a row at the top, and the rest gathered in
  [folders](#the-app-grid).

Both searches forgive typos, missing letters and initials in app names: `calxu` still finds
Calculator, and `mc` finds Mission Center. They find Power Off, Restart and the other actions too.

Besides apps, files and GNOME's settings, both searches answer from this machine, and nothing is sent
anywhere:

- **Sums and conversions**, as you type, in your language's numbers: `1284 * 0.18` gives 231.12,
  `200 + 10%` gives 220, `3.5 km in miles` gives 2.1748 mi and `21 °C in °F` gives 69.8 °F. `Enter`
  copies the answer. It converts lengths (mm, cm, m, km, in, ft, yd, mi, nautical miles), weights (mg, g,
  kg, t, oz, lb, stone), temperatures (°C, °F, K), data (bits and bytes, kB to PB, KiB to PiB) and time
  (ms to weeks). No currencies: their rates would have to come from the internet.
- **The time in other places**: `time in Tokyo` gives the time there now, with the day and how far ahead
  or behind you it is; `15:00 Paris in Tokyo` gives 23:00 in Tokyo, the same day, and `15:00 in Tokyo` starts
  from your own time. A place is any city or country the world clocks know, in your language or in English,
  a time zone such as `Europe/Paris`, or `UTC`, `UTC+2`. The clock is yours: 24 or 12 hours, as the top bar
  shows it. `Enter` copies the answer. It is worked out from this computer's own time-zone data.
- **Reminders**: `remind me at 17:00 to call the garage`, `remind me tomorrow at 9:00 to pay the rent`,
  `remind me in 20 minutes to take the pizza out`. The row shows the task and when; `Enter` makes it a task in
  [Notes](notes.md#reminders)' inbox with a reminder at that time, and the row under it does the same a day
  later. When Notes is locked, its quick note opens with the task in it and asks for the passphrase first.
  The words are English, as Notes' task box reads them, and a reminder needs a time.
- **Do it**: [Control](control.md)'s settings, found by their words. `Enter` on a row such as
  *Phone pairing: On* changes the setting at once; one that asks something or needs a password opens a
  terminal. *Control › …* opens it in Control.
- **Open windows**, by title or app, on any workspace: `Enter` switches to it.
- **The words you already know**: type what you would have called it, such as `recycle bin`, `task manager`,
  `control panel` or `print screen`, and a row takes you to the thing that does that here. It is a short table of
  everyday words, in your language and in English: `ame desktop search old-words off` turns it off.
- **Files by what you remember**: `pdf from last week`, `big videos`, `photos yesterday`, `lease from march`,
  `invoice over 2 mb`. A small set of rules reads kinds of file, dates and sizes from what you type, in your language
  or in English, and asks the file index GNOME already keeps; other words have to be in the file's name. It takes two
  words or more, so that a word alone is left to the apps. There is no model and no agent, it reads no file, and it
  keeps nothing. Words it cannot read just search as usual:
  `ame desktop search remembered off` turns it off.
- **The manual**: the pages of this manual that answer what you typed, by meaning as well as by words.
- **Clipboard history**, only when you type `clip:` first, and words after it to narrow it down:
  `clip: ssh`. `Enter` pastes the item where you were typing. It is the history
  [the clipboard](#the-clipboard) already keeps; the search stores nothing new.
- **The agent**, only while the [agentic features](ai-agent.md) are on: the last row, *Ask the agent:
  …*, opens your agent in a terminal with what you typed as its first question. Nothing starts before
  `Enter`, and the agent asks before it changes anything, as always.
- **Glance**: with the keyboard on a file Files found (`↓` to it), `Space` shows it without opening
  it. `↑` and `↓` go to the other files found; `Space` again closes it, and the search is still there.

Three more are offered and off until you turn them on:

- **Bookmarks**: Firefox's bookmarks whose name, address or folder hold what you typed; `Enter` opens one in your
  browser. They are read from a copy of Firefox's bookmarks file, which is deleted at once, and Firefox's history is
  never read. Firefox itself is left exactly as it is: `ame desktop search bookmarks on`.
- **Notes**: the titles of your [notes and tasks](notes.md) that hold what you typed, only while Notes is open and
  unlocked; `Enter` opens one in Notes. A note's text is never searched, and locked or closed, Notes says nothing:
  `ame desktop search notes on`.
- **Files by meaning**: files in your Documents folder found by what they are about, even when no word you typed is
  in them, with two words or more: *the letter about the tax refund* finds `refund-2025.pdf`. It uses the small
  model the [AI agent's](ai-agent.md) index already has, on the text GNOME's own indexer read from the files, and works
  out what each is about once a day, on mains power only, at low priority. What it keeps is in
  `~/.local/state/amethystora/file-meaning`: the files' names and numbers, never their words. Nothing is sent
  anywhere: `ame desktop search meaning on`. Off deletes what it kept.

Each can be turned on or off in Control's **Search** group or in a terminal:

```bash
ame desktop search                     # each, and whether it is on
ame desktop search answers off         # answers, times, reminders, settings, manual, windows, clipboard, agent,
                                       # glance, old-words, remembered, bookmarks, notes or meaning
```

## The app grid

`Super+A`, the app grid button on the dock, or a pinch with three fingers on the touchpad opens it. The apps you use
most sit in a row at the top, under **Recently used**, most used first and as many as fit in a row (eight at most). At
the very first login it holds only the apps that opened by themselves, the Manual and Control, until you use others;
an account that has never opened an app has no such row. Under **All apps** come the folders and the apps that stay
loose.

The folders fill themselves from what each app says it is: **Office**, **Graphics**, **Games**, **System**,
**Development** and **Utilities**, named in your language. An app that is two of these is in both folders. Security,
Updates, Control, Backups, the Manual and Phone stay loose, and so do Steam and Lutris. The folders are GNOME's own:
rename one, take an app out or move the icons, and that is kept for you. If you had changed your folders before, yours
stay as they were. A second page starts only once the first is full: the icons stay in the order you gave them, and a
gap on a page closes.

```bash
ame desktop app-grid recent off        # no row of recent apps; on brings it back
ame desktop app-grid folders reset     # the folders as the image has them, forgetting your changes
```

### Hide, rename or change an app

Right-click an app in the grid, a folder or the search. **Rename…** changes the name under its icon, and only that:
its launcher stays as it is, and the search finds it by both names. **Change icon…** shows the same app as each of the
installed icon packs draws it, and **A picture of your own…** takes a picture (PNG, JPEG, SVG or WebP), which is made
into an icon of its own in `~/.local/share/amethystora/app-icons`; **Reset** gives it its own icon back. The new name and
icon show in the grid, the dock, the search and the window switcher. An app whose icon is its maker's logo has no other
pack's drawing to choose: no pack draws another maker's logo.

**Hide from the grid and search** takes it out of both at once, with **Undo** in the notice. A hidden app still opens
from a terminal and from **Open With**, and stays in the dock if it is pinned there; shown again, it comes back at the
end of the grid, as a newly installed app does. **What can it reach?**, on a
Flatpak app, opens Security's Apps page on that app: what it can reach beyond its sandbox, and what Security can change
about it. Nothing in the menu changes a permission.

Control's **Apps** page lists every app you hid, renamed or gave another icon, each with the way back. Nothing is
written but the app grid's own settings: no launcher of your own, which the [security watcher](security.md) would read
as something planted.

```bash
ame desktop app hide org.gnome.Calculator.desktop                    # show brings it back
ame desktop app rename org.gnome.Calculator.desktop "Sums"           # without a name, its own again
ame desktop app icon org.gnome.Calculator.desktop amethystora-neon   # reset, or picture FILE
```

### The app grid in order

The grid keeps the icons where you put them, as GNOME does. Control's **The app grid's order** (on the Desktop page)
sorts it instead: **A to Z**, **By use**, the apps you use most first, from GNOME's own record of use, kept on this
machine, or **By kind**, from what each app says it is (work, pictures and sound, development, games, then the
system's). Your folders stay first, as you left them. Going back to **Where you put them** gives your own order back as
it was.

```bash
ame desktop app-grid order name        # custom, name, use or kind
```

## The clipboard

`Super+Shift+V` opens the clipboard history: the last 15 things you copied. Pick one to copy it
again, type to search, and pin the ones you keep coming back to. It is also the clipboard icon in
the top bar.

**Text actions and QR codes.** The ⋯ button of a text in the history (or `A` on it) opens **Make a new copy**:
the text as **Pretty JSON**, in **UPPER CASE**, **lower case** or **Title Case**, with its spaces trimmed, on
**One line**, or with a web address's `%20`s decoded, each one a new item at the top of the history and on the
clipboard, ready to paste, and the text's characters and words counted. The original stays as it was. For a link
or a short text, **Show as QR code** shows it as a code any phone's camera reads, to hand it to a phone that is not
paired, or to somebody else's. While the screen is shared or recorded the code is not shown.

The history is never on disk as you copied it: text and pictures alike are sealed (encrypted) in
`~/.local/state/amethystora/clipboard`, under a key kept in your login keyring, which your password
protects. A copy of the disk, a snapshot or a stolen laptop shows nothing of it without that password.
It is left out of [backups](security.md#backups). A keyring given no password, as some machines that sign
in by themselves have, keeps its key in the clear, and then the history is only as safe as the disk.

How long it is kept is yours to choose, in [Control](control.md)'s **Clipboard history** or in a terminal.
Pinned items are always kept, and the history outlasts locking the screen either way.

```bash
ame desktop clipboard always    # everything, until you clear it (the default)
ame desktop clipboard restart   # pinned items, and the rest until the computer restarts
ame desktop clipboard session   # pinned items, and the rest until you log out
```

Nothing is recorded of what a password manager marks secret, as KeePassXC does, or of what you copy
in KeePassXC, Secrets, Bitwarden, 1Password or Proton Pass. Before copying anything else secret, turn on
**Private mode** in the same menu, and nothing is recorded until you turn it off.

After logging in with a fingerprint or a security key, your keyring stays locked, so the menu shows
what you copy from then on, under **Unlock to see earlier items**. Click it and type your password to
see the rest. The history is sealed under this machine's keyring: on another machine, or after an
administrator resets your password, it does not open, and it is kept aside as `history.unreadable`.

## The shelf

The shelf is for moving things between two apps that are never on screen together: put them down, go where they
belong, pick them up. It is off until you turn it on in Control (**The shelf**, on the Desktop page) or with
`ame desktop shelf edge`.

Start dragging a file, some text or a picture, and a glow appears at the right edge of the screen. Let go on it,
and the shelf opens there: a narrow column that stays above your windows, on every workspace, until it is empty.
Drop more onto it whenever you like. Then drag an item out into any app; it leaves the shelf once the app has taken
it. Hold **Ctrl** while you drag to keep it on the shelf as well. **Clear** empties the shelf, and the shelf closes
when the last item goes.

If you would rather not aim for the edge, `ame desktop shelf shake` opens it wherever you are: shake the pointer from
side to side while dragging, and let go on the card that appears.

The shelf is a window, so Alt+Tab reaches it. **Delete** takes the item in focus away, **Ctrl+Delete** clears the
shelf, and a right click (or the Menu key) on an item has **Remove**, **Show in Files** for a file, and **Clear the
shelf**.

A file stays where it is: the shelf holds a reference to it, and dragging it out gives the app a copy, never a move,
even onto a folder in the [dock](#what-the-dock-holds). If the file is moved or deleted meanwhile, its item turns
grey. Text and pictures are kept in memory only, by the shelf and, while the first drop is handed over, for a moment
by the desktop: neither saves them anywhere, and they never reach the [clipboard history](#the-clipboard). The
shelf lets go of them when they leave it, when it is cleared, or when you log out. If the shelf or the desktop
crashed meanwhile, its crash report could hold them; `ame security no-coredumps` keeps crash reports from being
written. The shelf takes a drop of up to 1 MB of text or of a list of files, or a PNG picture of up to 32 MB and 64
million pixels, and holds 32 items; it says so when it cannot take something.

## Emoji and symbols

`Super+.` opens emoji and symbols over the window you are typing in, under the text cursor. Type to
search, in your language or in English (`euro`, `arrow`, `smile`), or pick a tab: **Recent**,
**Smileys**, **Arrows**, **Maths**, **Money**, **Punctuation**, and the rest of Unicode's emoji. `Enter`
or a click puts the one picked where you were typing; `Escape` closes it without typing anything. The
arrow keys move in the grid, `Tab` goes between the search, the tabs and the grid, and
`Ctrl+Page Down` and `Ctrl+Page Up` change tab.

What you pick is typed straight in where the app allows it. Otherwise it goes through the clipboard and
is pasted with `Shift+Insert`, or `Ctrl+Shift+V` in a terminal, and then it stays on the clipboard.

Hands and people come in the skin tone you choose: right-click one, hold it, or press the `Menu` key on
it for the six tones, and the one you pick becomes the tone for the rest. It is also in Control's
**Typing** group, or:

```bash
ame desktop emoji tone medium    # none, light, medium-light, medium, medium-dark or dark
```

Your recent picks are kept on this computer only, in `~/.local/state/amethystora/emoji.json`. The lists
and the names come from Unicode's emoji list (Emoji 16.0) and the Unicode CLDR, under the Unicode
License. Smile, the emoji app, is still on `Ctrl+Alt+Space`. IBus's own emoji panel has no shortcut, so that
`Super+.` reaches the picker in every text field.

## Text shortcuts

A text shortcut turns a few characters into a longer text as you type: `;addr` and a space becomes your
address, `;sig` your signature, `:shrug` becomes ¯\\\_(ツ)\_/¯. Add them in Control's **Typing** group,
which also has a field to try them in, and turn on **Expand text shortcuts** there, or in a terminal:

```bash
ame desktop text-shortcut add ';addr'    # then type the text, and Ctrl+D on a line of its own
ame desktop text-shortcuts on
```

`ame desktop text-shortcut list` lists your shortcuts, `ame desktop text-shortcut remove ';addr'` takes
one away, and adding a shortcut you have already gives it the new text (Control asks first). You can
also give the text after the shortcut, `ame desktop text-shortcut add ';addr' '4 Amethyst Lane'`, but then
every account on the computer can see it in the list of running programs while the command runs; typed
or piped in, and from Control, it never is.

Type the shortcut where a word starts, then a space. Start shortcuts with a character you seldom type
before a word, like `;` or `:`, so that they never expand by accident. Write `{date}` in a text for
today's date. Your app's Undo takes an expansion back. A shortcut is 2 to 32 characters with no spaces,
a text up to 2000 characters, and you can have 200.

It is off until you turn it on. While it is on, the desktop looks at what you type in apps for your
shortcuts: it keeps only the last few characters, as many as your longest shortcut, in memory, and
never writes them anywhere. It forgets them when you switch windows, when a field of another kind (a
password field, say) or one of the desktop's own takes the keyboard, and at every key that is not text,
such as an arrow key, Enter or Tab. It cannot tell when you click into another field of the same window,
so it keeps them then until one of those happens. Outside terminals it only replaces a shortcut it can see
the app holds, so what you typed in one field is never replaced in another. It never looks at password or PIN
fields, at fields for numbers, phone numbers or dates, at the desktop's own fields, or at what an input
method like Pinyin or Mozc is composing. A key you press while a shortcut is being replaced stops the
replacing. Your input sources and input methods stay as they are.

It works in apps that use GNOME's input method and tell it the text they hold: GNOME's apps,
Amethystora's, Firefox and most others. In terminals, which do not tell it, a shortcut is replaced with
BackSpace keys, and only one whose text has a single line, since a new line would run the command; if you
paste into a shortcut halfway, or finish one in another pane of the same terminal window, what you pasted
or typed there can be taken for part of it. Other apps that do not tell
it their text get no expansion, since there a click could have moved the cursor unseen. It does not work
in apps that run through X11 (older apps, games, Wine) or that talk to IBus themselves.
The shortcuts are in `~/.config/amethystora/text-shortcuts.json`, and only you can read it. They are not
part of your setup file, since they are your own writing.

## The keyboard

Control's **Keyboard** page (`amethystora-control keyboard`) has every keyboard shortcut and the keyboard
settings GNOME's Settings does not show.

### Every shortcut

The **Shortcuts** card lists GNOME's shortcuts, the desktop's, the tiling's hundred and your own custom
ones in one list, with a field to find one by what it does or by its keys. Each says whose it is, or
**Yours** once you changed it. Two shortcuts on the same keys are marked in red with the other one's name,
unless both are as Amethystora ships them: the tiling takes some of GNOME's keys while it is on, on
purpose. A tiling shortcut works only while the tiling is on.

**Change** waits for the new keys: press them, `Escape` to give up, or `Backspace` to turn the shortcut
off. So that keys the desktop already uses reach it, Control holds the desktop's own shortcuts back while it
listens; GNOME asks you once whether it may. **Reset** puts back the keys it came with. A shortcut needs `Super`, `Ctrl` or `Alt`, unless it is a
function key, `Print` or a media key, so that no key stops typing. What you set is written where GNOME's
Settings and the tiling's own window read it, and your [setup file](setup.md) keeps it. In a terminal:

```bash
ame desktop shortcut list
ame desktop shortcut set org.gnome.settings-daemon.plugins.media-keys previous '<Super>Left'
ame desktop shortcut clear org.gnome.settings-daemon.plugins.media-keys previous
ame desktop shortcut reset org.gnome.settings-daemon.plugins.media-keys previous
```

A custom shortcut is named as `custom` and its id: `ame desktop shortcut set custom custom20 '<Super>t'`.
The AI agent may never change the shortcuts that open it.

### Keyboard settings

| Setting | What it does | In a terminal |
| --- | --- | --- |
| **Caps Lock acts as** | Escape, Ctrl, Backspace, nothing, or Caps Lock | `ame desktop keyboard caps escape` |
| **Compose key** | Right Alt, Right Ctrl, Menu, Right Super or Caps Lock starts a composed character: Compose, then `'` and `e`, types é | `ame desktop keyboard compose ralt` |
| **Super on its own opens** | The overview, the [search box](#finding-things), or nothing; Super's shortcuts work either way | `ame desktop keyboard super search` |
| **Middle click pastes** | The middle button pastes the last text selected, anywhere | `ame desktop keyboard middle-paste off` |
| **Num Lock on at start** | On at every start, rather than as it was left | `ame desktop keyboard numlock on` |
| **Repeat after** | How long a key is held before it repeats, 150 to 1000 ms | `ame desktop keyboard repeat-delay 400` |
| **Each window keeps its keyboard layout** | See [below](#a-layout-for-each-window) | `ame desktop keyboard per-window on` |

`ame desktop keyboard` alone says how each is set. The Caps Lock and Compose settings change only their own
part of GNOME's keyboard options and keep the rest. All of them are in your setup file.

### A layout for each window

With more than one keyboard layout (GNOME's Settings, **Keyboard**), each window can keep its own: Arabic
in a chat, French in work mail, English in the terminal, each coming back when you return to that window.
Turn on **Each window keeps its keyboard layout** on the Keyboard page, or `ame desktop keyboard
per-window on`. `Super+Shift+Space` still switches the layout of the window in use.

### Paste as plain text

`Super+Alt+V` pastes what is copied without its fonts, colours and links, into any app, whatever its own
paste does; `Ctrl+V` pastes as before. The text is typed in where the app allows it; otherwise it is put
back on the clipboard as plain text and pasted. A password that a password manager marks secret is never
put back on the clipboard, so the clipboard's history still keeps no copy of it. Change or turn off the
key on the Shortcuts card.

### Hold a key for accents

Hold a letter, and its accented forms appear in a strip over the text cursor: hold `e` for é è ê ë ē ė ę.
Press `1` to `8`, or the arrow keys and `Enter`, to type one in place of the letter; `Escape`, or any other
key, leaves the letter as it is. For writing French, Spanish, German, Polish or Turkish on an English
keyboard.

It is off until you turn it on: **Hold a key for accents** on the Keyboard page, or `ame desktop accents
on`. While it is on, a held letter no longer repeats in the apps it works in. Like text shortcuts, it looks at
the keys you type in apps, keeps nothing of them but the letter held, and never looks at password fields. It
works in apps that tell GNOME's input method their text (GNOME's apps, Amethystora's, Firefox and most
others), and not in terminals, fullscreen windows such as games, or apps that run through X11. It is not part
of your setup file.

## A resting screen

With **A resting screen** on (Control, **Desktop**, **Resting**, or `ame desktop resting-screen on`), a computer left
alone for a few minutes rests: a large clock and the day over your wallpaper, which drifts slowly, what is playing if
something plays, and when the screen will lock. The windows are hidden under it. Any key, a click or moving the pointer
brings everything back, and that key never reaches a window. The screen locks when it would have anyway: the resting
screen neither holds it back nor brings it forward.

It does not come while something keeps the computer awake (a video playing, a presentation), while a window is
fullscreen, or while the screen is shared or recorded. On battery, in the power saver and with reduced motion the
wallpaper holds still. Choose how long the computer is left alone first, 1 to 30 minutes:

    ame desktop resting-screen after 5

## Screenshots

`Print` opens GNOME's screenshot bar. Above it, for an area or the whole screen, are tools to draw on the
picture before it is saved: move a mark, a pen, a box, a filled box to hide something, a highlighter, an
arrow, text and numbered markers, in eight colours and the width the slider (or the mouse wheel) sets.
`1` to `9` pick the tools, `Ctrl+Z` undoes, `Delete` removes the mark you picked up. `Ctrl+C` copies the
picture without saving it and `Ctrl+S` saves it where you choose.

Once it is taken, a thumbnail rests at the bottom right for six seconds, longer while the pointer is on it.
Click the picture to open it in your image viewer, or use its buttons: **Edit** opens it in Gradia, **Copy**
copies it again, **Pin on top** pins it (below), **Show in Files** shows it in its folder and **Delete** moves
it to the trash. `ame desktop screenshot thumbnail off` (or Control, **Desktop**, **Screenshots**) puts GNOME's
own notification back instead.

### Pinned pictures

A pinned picture stays above every window, on every workspace, while you work in the window below: numbers
from a chart, a recipe, a design to match. Pin one from the thumbnail, or straight from the screenshot bar:
frame an area and click **Pin on top** beside the pointer button (or press `Ctrl+P`), and it stays exactly
where it was, without being saved or copied.

- Drag it to move it; scroll over it to make it larger or smaller; `Ctrl` and scroll make it see-through.
- The arrow in its corner lets clicks pass through it to the window below, for tracing; its ring turns amber.
  Click the arrow again to take it back.
- The cross, or `Esc` after clicking it, lets it go. Right-click it to copy it, save it, or pick **Solid**,
  **See-through** or **Faint**.
- Up to six at a time; a seventh lets the oldest go. They are kept while the screen is locked and are gone
  when you log out.
- While you share or record the whole screen, pinned pictures and the thumbnail are blurred, for everyone and for
  you, and carry a small mark: a picture of another window could hold what the share should not show. They are
  sharp again when the share ends.

### Trim a recording

With **Trim a recording** on (Control, **Desktop**, **Screenshots**, or `ame desktop screenshot trim on`), the notice
that a screen recording was saved offers **Trim**. It opens the recording as a strip of frames: drag its two ends (or
use the **Start** and **End** sliders), and the frame at the end you move shows above. The times under the strip say
where the part starts and ends and how long it is. **Save as WebM** keeps that part as a video, and **Save as GIF** as
a short GIF for a chat or an issue, both beside the recording, as `NAME (trimmed).webm` or `.gif`; **Show in Files**
shows it. The recording itself is never changed and nothing is written over. To trim any recording from a terminal:

    amethystora-trim ~/Videos/Screencasts/recording.webm

`ame desktop screenshot pin off` turns pinning off and lets go of every pinned picture.

## Copying text from the screen

Text in a picture, a video call, an error dialog that cannot be selected, a scanned page: Gradia reads it
and copies it for you, on your machine.

1. Press `Print` and drag over the text.
2. Click **Extract Text** in the screenshot bar, or press `Ctrl+E`.
3. The first time, Gradia asks for a language. Pick it: this is the one download, of that language's
   data from GitHub, and it is yours to agree to. After it, nothing leaves the machine.
4. The text is copied at once, and Gradia says **Copied!**. Paste it anywhere; it is in the
   [clipboard history](#the-clipboard) like anything else you copy.

Good to know:

- The button and `Ctrl+E` work on an area only. They are off when you choose a window, the whole screen or a
  recording.
- The capture is saved like any screenshot, in your screenshot folder. Delete it there if you do not want it.
- The language data lives in `~/.var/app/be.alexandervanhee.gradia/data/tessdata`. Open Gradia's
  preferences to add another language.
- Gradia is one of the apps every machine comes with; if you removed it, install it again from Bazaar
  and the button comes back.

## Live captions

Live captions put the words of whatever is being said in a video, a call or a podcast in a dark box near the bottom
of the screen, as they are spoken. They are made on this computer from the sound it plays, by the Vosk speech
recognizer, and they are **off**: turn them on with

```bash
ame desktop captions on
```

or **Live captions** on [Control](control.md)'s **Appearance** page. The first time, it says what it downloads and
asks: a speech model of 39 MiB (68 MiB once unpacked), fetched once from alphacephei.com, the makers of Vosk, and
checked against the copy the image knows. After that, **Live captions** in [quick settings](#quick-settings) turns
them on and off.

Good to know:

- Captions hear **English** unless you choose another language: French, Spanish, German, Italian, Turkish,
  Portuguese, Dutch, Russian or Polish, with **Captions language** in Control or
  `ame desktop captions language fr` (`es`, `de`, `it`, `tr`, `pt`, `nl`, `ru`, `pl`; `en` is English). Each
  language's speech model, 31 to 51 MiB, is downloaded once from alphacephei.com when you choose it, after it says
  so and asks, and checked against the copy the image knows; then that language works offline like English.
  [What this machine sends](security.md#what-this-machine-sends) names every language downloaded.
  [Dictation](#dictation) hears the same language.
- The sound is listened to only while captions are on and an app plays something, and only in memory: it is never
  recorded, written down or sent anywhere, and neither are the words. They are gone a few seconds after they are
  shown.
- Your microphone is never used. The **Live captions** toggle in quick settings, checked, is what shows they are on.
- The box is drawn over every window, a video in full screen too, and clicks go through it. It is part of the
  screen, so a screenshot taken or a screen shared while words show has them in it.
- They cost a part of one processor core while speech plays and hold about 230 MB of memory while they are on.
  Silence, and an app that is paused, cost next to nothing.
- Recognition is good for clear speech and makes mistakes with music, accents and several voices at once.
- `ame desktop captions off` turns them off; `ame desktop captions remove` turns them and dictation off and deletes
  every model; `ame desktop captions status` says how things stand.

## Dictation

Dictation types what you say into the text field you are in: a note, a message, a search. It is made on this computer
by the same recognizer as [live captions](#live-captions), in their language, and it is **off**: turn it on with

```bash
ame desktop dictation on
```

or **Dictation** on [Control](control.md)'s **Appearance** page. If the speech model of the captions' language is not
on this computer yet, it says what it downloads and asks first, as captions do.

Then click into a text field, press **Super+Ctrl+Shift+D** and speak. A small pill near the bottom of the screen says
*Listening on this computer only* and shows the words as they come; each sentence you finish is typed into the field.
Press **Super+Ctrl+Shift+D** again to stop. It also stops by itself after 15 seconds without a word, after two
minutes, or when you go to another window.

Good to know:

- The microphone is listened to only while the pill shows, and GNOME's microphone indicator shows in the top bar
  meanwhile. Nothing is recorded or written down, and nothing is sent anywhere.
- It never types into a password field.
- Most apps take the words as typed text. In a terminal, and in an app that does not take text from the input method,
  each sentence is pasted instead, so it also lands on the [clipboard](#the-clipboard) and in its history.
- Dictation writes what it hears without punctuation, and makes the mistakes captions make with noise and accents.
- `ame desktop dictation off` turns it off, and its shortcut with it.

## Gestures and keys

Each card moves the way the gesture or the keys do. Click one, or press `Space` on it, to pause it; with animations
off they stay still. Shake to find is off until you turn it on ([Pointers](themes.md#pointers)).

![Shake to find](media/shake-to-find.webm) ![Pinch for the apps](media/pinch-apps.webm) ![Search anything](media/super-space.webm) ![Hold Super](media/hold-super.webm)
![Switch windows](media/super-tab.webm) ![Snap layouts](media/snap-layouts.webm) ![Spread for the desktop](media/spread-desktop.webm) ![Four fingers for Today](media/four-left-today.webm)

## Touch and pen

For a computer you touch or draw on, Control's **Desktop** page has a group **Touch and pen**, and `ame desktop touch`
alone shows how each is set. Both are off until you choose them.

### Touch mode

The desktop sized for a fingertip: the top bar 12 px taller with larger text, the dock's icons up to 64 px, menus with
rows at least 44 px high, and **Today from the right edge**: a handle there, and a swipe in from that edge opens the
calendar menu. Apps keep their own sizes.

- `ame desktop touch mode follow` turns it on while the computer is folded into a tablet: Mutter tells by the
  hinge's tablet switch, or by a touch screen with no mouse or touchpad where there is no switch. GNOME's on-screen
  keyboard follows the same signal by itself.
- `ame desktop touch mode on` keeps it on; `off` turns it off.

While it is chosen, quick settings has a **Touch mode** tile. Some hinges report the wrong thing: a click on the tile
changes it until the hinge next moves (or until you log out), without changing your choice.

### The pen ring

Six tools at the pen's tip: **Scribble** (drawing on the screen, where present mode offers it), **Area** (a screenshot
of an area), **Note** (a quick note into Notes), **Text** (the text in an area, copied, with Gradia), **Right click**
and **Back**, each run where the ring was opened. Move the pen toward a tool to light it, then touch the screen, or
carry on past the ring's edge. A touch in the middle, `Escape` or a click outside closes it.

- `ame desktop touch pen-ring ring` turns the ring on; `strip` shows the tools in a column at the screen's edge
  instead; `off` turns them off. `Super+Ctrl+P` opens them.
- The pen's side button opens them once it sends that shortcut: `ame desktop touch pen-button on` sets it up for each
  pen whose settings GNOME has or that has touched the tools, and for pens without a serial number of their own on the
  tablets attached now, keeping what each button did; `ame desktop touch pen-button off` gives that back, where the
  button still does what it set. A pen with a serial number of its own (most Wacom pens) is known only once it has
  been used: open the tools with `Super+Ctrl+P`, touch them with the pen and run it again, or choose `Super+Ctrl+P`
  for its button in Settings, under **Wacom Tablet**. The command says which it set.
- `ame desktop touch pen-tools area note text right-click back` picks the tools and their order, from the top,
  clockwise.

## Tips

The first time a feature can help, a small card next to it says how, with **Got it**: shaking the mouse to find the
pointer, holding `Super`, `Super+Space`, snap layouts, the Scripts menu of Files, sending a file to your phone, and
the pinch on the touchpad. Each tip comes once, ever. None comes in the first hour after you start using Amethystora,
never two at once, and at least half an hour apart; none while a window is fullscreen, while the screen is shared or
recorded, while notifications are silenced, or over the overview or a dialog. A tip never takes your keyboard or your
clicks, and closes by itself after twelve seconds.

```bash
ame desktop hints off      # no tips; on brings them back
ame desktop hints reset    # the tips you have seen can come again
```

## The dock

The dock holds your pinned apps, the ones that are running, the last few you used, your folders and the trash.
`Alt+1` to `Alt+9` switch to the pinned app in that place, and open it if it is not running; hold `Super` for a
second to see the numbers on the dock. Right-click an app to pin or unpin it, or drag it off the dock: it dissolves,
and stays in the app grid. Right-clicking one of Amethystora's own apps also offers its pages: **Hotkeys** in the
Manual, **Tasks** in Notes, **Crashes** and **Apps and Services** in Logs, **Backup Status** in Backups, **Virus
Scan** in Security, and **Appearance** and **System** in Control.

An app's icon, on the dock and in the app grid alike, shows how many notifications it has waiting,
a bar while it is busy with something long, and is greyed out while the app is being updated. Do
Not Disturb hides all of that.

### How the dock moves

The icons swell in a wave under the pointer. Choose **Lift** to raise only the icon under the pointer, or **Still**,
in [Control](control.md)'s **Dock** settings or in a terminal:

    ame desktop dock magnify lift      # or swell, off
    ame desktop dock amount 180        # how far Swell grows, 120 to 220 per cent
    ame desktop dock size 56           # the icons at rest, 40 to 64 pixels; reset for the dock's own choice

A dock too long for its screen draws its icons smaller than the size you picked, so that every one fits.

While an app starts, a ring runs round its icon until its window opens (`ame desktop dock launch hop`, `breathe`
or `off` for another sign or none). A bar under an icon marks a running app, twice as long when it has two windows
or more (`ame desktop dock indicator dot` for a dot per window, `glow` for a light under it).

### What the dock holds

After the running apps come the last three apps you used that are neither pinned nor running (`ame desktop dock
recent off` to leave them out). GNOME's own record of app use fills them at login, and again after the screen was locked, since
the dock remembers the order you used them in nowhere but in memory; with **Usage history** off in Settings ›
Privacy, there are none.

Rest the pointer on a running app for a moment to see its windows above it, and click one to bring it forward
(`ame desktop dock previews off` to see only names). Right-click it, or hold it, to fan all its windows out over
its menu: scroll or press Left and Right to go through them, and click one or press Space.

Drop one pinned app on another and wait until it lights up, to make a folder of them. Click the folder to open it,
and click its name to rename it; **Take the folder apart** in its menu puts the apps back.

Your Downloads folder is a stack: click it to see the twelve newest things in it, newest first, and **Open in Files**
for the rest. While files arrive in Downloads, its stack counts them (**+3**) and hops for each; the count is gone once
you open the stack. It is kept in memory only. Drag a folder from Files onto the dock to keep it there as a stack too,
and drag a stack off the dock to take it away. Files dropped on a stack are moved into that folder, or copied while you hold `Ctrl`, by Files, which
shows how far it got and can undo it. Files dropped on **Phone** go to your paired phone.

    ame desktop dock stack add ~/Documents
    ame desktop dock stack remove ~/Documents

While a Flatpak app is being updated, by Bazaar, `flatpak update` or the automatic updates, its icon on the dock and in
the app grid is at half strength. You can still start it: it runs the version it has, and the new one the next time it
opens. `ame desktop dock updating off` leaves the icons as they are.

### Where the dock lives

The dock is at the bottom of the main screen, and shows the apps of every window. Put it on the left or right, or on
every screen, where each screen's dock shows the apps whose windows are on that screen beside the pinned ones:

    ame desktop dock edge left          # or right, bottom
    ame desktop dock screens every      # or primary, the main screen only

To have it out of the way, `ame desktop dock autohide touch`: windows then use the whole screen, and the dock slides
away while one touches it. Push the pointer against that edge to bring it back. Where a screen continues on another
past that edge, GNOME cannot keep a strip free there, so that screen's dock always hides that way.

`ame desktop dock workspace-apps on` keeps only the apps of the workspace you are on, besides the pinned ones. A click
on a pinned app whose windows are on another workspace takes you to them.

## The Amethystora menu

`Super+Alt+Space` opens one menu for the things that are otherwise scattered over Settings, the
terminal and [`ame`](terminal.md#ame):

| Entry | Does |
| --- | --- |
| Ask an agent | Opens the [AI agent](ai-agent.md), while it is on |
| Diagnose a problem | Has the agent find out why something broke, while it is on |
| Make a widget | Has the agent make a [widget](widgets.md) for the top bar, while it is on |
| Theme | The theme picker |
| Background | The wallpapers of the current theme |
| Keybindings | The hotkeys, in the terminal |
| Control | Amethystora's own settings, in [a window of their own](control.md) |
| Manual | This manual |
| Web apps | Install or remove a [web app](web-apps.md) |
| Apps and system commands | Every `ame` command, in its group |
| System | Lock, log out, restart, shut down, check for updates, the security report, the logs and backups |

Type to filter or move with the arrow keys, `Enter` to choose; `Esc` leaves the filter, and a second `Esc` goes back. From
a terminal, `amethystora-menu` opens it, and `amethystora-menu theme`, `background`, `keys`, `webapps` or `system`
opens that part of it.

## Tiling windows

The desktop arranges the windows for you. Each workspace is a strip of windows standing side by side at
full height, and the strip can be wider than the screen. A new window opens to the right of the one
you are on, windows never overlap, and the strip scrolls to keep the focused window in view, which a
frame around it marks:

```
  ┌────────┬──────────────┬──────────────────────┬────────┐
  │  Mail  │   Terminal   │       Browser        │  Chat  │
  └────────┴──────────────┴──────────────────────┴────────┘
           ╰──────────── the screen ─────────────╯
```

Mail and Chat are still open, just off the edge. `Super+Left` scrolls back to Mail. Push the pointer
against the left or right edge of the screen to see what is beyond it, and click a window there to
go to it. On a touchpad, swipe sideways with three fingers to slide the strip, up to open the overview,
and down to see the workspaces you have been on as a pile of cards. Drag a window by its title bar and
drop it where you want it in the strip, or against a screen edge to see where it will go first; the
mouse wheel moves along the strip while you hold it. A window you resize by its edge onto another
screen floats.

The keys follow one pattern. `Super` and an arrow moves to another window. Add `Ctrl` and the window
moves instead. Add `Shift` and you go to another monitor.

### Moving around

| Key | Does |
| --- | --- |
| `Super+Left` / `Super+Right` | The window to the left / right |
| `Super+Up` / `Super+Down` | The window above / below, when windows share a column |
| `Super+Home` / `Super+End` | The first / last window on the workspace |
| `Super+[` / `Super+]` | Slide the strip; the window most in view is the one you land on when you let go of `Super` |
| `Super+Page Up` / `Super+Page Down` | The previous / next workspace, shown as cards in a row |
| `Super+Alt+Left` / `Right` / `Up` / `Down` | Swap this monitor's workspace with the one the monitor in that direction shows |
| `Ctrl+Alt+Shift` and an arrow | Move this monitor's workspace to the monitor in that direction |
| `Super+Shift` and an arrow | The monitor in that direction |

### Several monitors

With two monitors or more, each shows a workspace of its own, and a workspace is on one monitor at a
time. The monitor the pointer is on is the one you work on: move the pointer to the other monitor and
its workspace is the current one, with no animation. `Super+Page Up` and `Super+Page Down` go
through the workspaces of the monitor you are on and the empty ones, and the one you choose stays on
that monitor. A workspace chosen another way (`Super` and a number, the workspaces in the top bar, an
app's window) comes up on the monitor it was last on, and the pointer goes there; when no monitor showed
it, the monitors slide to it as GNOME's do. Swapping or moving a workspace between monitors needs one
workspace more than you have monitors. If you chose dynamic workspaces in Settings (Multitasking), they
stay dynamic: a workspace a monitor shows is kept even when it is empty, and there is always one
more than you have monitors. When you unplug the monitor you are working on, the most recent workspace
of the other monitors becomes the current one, and a monitor plugged back in shows the workspace it had,
if it is still there.

The top bar and the dock start on the main monitor only. `ame desktop top-bar screens on` gives every other
monitor a top bar of its own, and `ame desktop dock screens every` a dock of its own; Control has both.
Such a bar is as tall and as round as the main one, with the
workspaces (the one that monitor shows lit) and the clock. Click a workspace there to show it on that
monitor; one another monitor shows swaps places with it. Drop a window from the overview on a workspace
to send it there. The tray, quick settings and the menus stay on the main monitor's bar, and a bar you
hide with `Super+Ctrl+B` hides on the monitor showing that workspace. In the classic layout, where the
other monitors keep their windows on every workspace, their bars show the clock alone.
`ame desktop top-bar screens off` takes the other monitors' bars away again.

### Arranging

| Key | Does |
| --- | --- |
| `Super+Ctrl+Left` / `Super+Ctrl+Right` | Move the window, with the whole column it is in, left / right along the strip |
| `Super+Ctrl+Up` / `Super+Ctrl+Down` | Move it up / down in its column |
| `Super+I` | Pull the window on the right into this column, below this one |
| `Super+O` | Push the bottom window of this column out into a column of its own |
| `Super+Shift+O` | Push this window out into a column of its own |
| `Super+Ctrl+Page Up` / `Page Down` | Move it to the previous / next workspace |
| `Super+Shift+Ctrl` and an arrow | Move it to the monitor in that direction |
| `Super+T`, keep `Super` held | Carry the window along as you move around, and drop it where you let go |

### Sizing

| Key | Does |
| --- | --- |
| `Super+R` | Step the width through 38%, 50% and 62% of the screen less its margins (`Super+Alt+R` steps back) |
| `Super+Shift+R` | The same for the height of a window in a column (`Super+Alt+Shift+R` steps back) |
| `Super` and `+` / `-` | Widen / narrow the window a step; with `Shift` as well, its height |
| `Super+F` | Fill the width of the screen, and back; maximising a window does the same |
| `Super+C` | Centre the window on the screen |
| `Super+Shift+C` | Where the strip scrolls a focused window to: just into view, the centre or the edge |
| `Super+Shift+W` | Where new windows open: to the right, to the left, or below this one |

### Floating windows

Dialogs float above the strip on their own. `Super+Ctrl+Escape` lifts the focused window out of the
strip so that it floats as well, and puts it back; every window's menu has the same as **Float above the
strip**, a dialog's too. `Super+Alt+Escape` hides every window floated that way at once, and brings them back.
A window you put on every workspace (**Always on Visible Workspace** in its menu) leaves the strip and
stays where it is.

### And the rest

| Key | Does |
| --- | --- |
| `Super+N` | A new window of the app you are in |
| `Super+Ctrl+B` | Hide or show the top bar on this workspace |

### Tiling settings

**Tiling settings** on [Control](control.md)'s **Desktop** page, or `ame desktop tiling-settings`,
opens the tiling's settings window. Every change in it applies at once.

- **Windows**: the gaps between windows and around the screen, the frame around the window you are on,
  where new windows open and which places `Super+Shift+W` goes through, how a workspace scrolls to the
  window you go to, the widths `Super+R` and the heights `Super+Shift+R` step through (shares like
  `38%, 50%, 62%`, or widths like `800px`), the width `Super+F` gives, and whether maximizing a window
  fills the width or the whole screen.
- **Motion**: how long a window takes to glide, the speed of `Super+[` and `Super+]`, the map of the
  strip and the darkening of the other windows while you move around, what the screen edges do (show the
  window beyond, go to it on a click, or after the pointer stays there), the touchpad's swipes and their
  fingers, speed and friction, the two icons in the top bar, and the size of the windows in `Alt+Tab`.
  Off as the desktop comes, for whoever wants them: the workspace's name in the top bar (a click opens
  the overview, the other button renames it), a line under the top bar marking where the window is in
  the strip, and an overview with only the floating windows or without them.
- **Workspaces**: a picture for every workspace in place of the theme's wallpaper (or none, only each
  workspace's colour), and for each workspace a name, a picture, a colour and a folder of its own. The
  colour frames the workspace's card while you go through the workspaces, and apps started while the
  workspace is shown start in its folder.
- **Keyboard**: every shortcut, including the ones the image leaves unbound (the next window, the n-th
  column, the window above or else the previous workspace, centring both ways, closing, and a switcher
  that goes through the windows used last, drawing each where its strip would show it). Click one and
  press the new keys. The first time, GNOME asks whether the window may have the keys the desktop uses
  while you press them: allow it. A shortcut another one uses as well is marked.
- **Window rules**: a window picked by its class or its title as it opens can float, open at a width,
  or open on a workspace, once or every time. `/a pattern/` matches part of a name, and `*` any window
  no other rule picks.

`gsettings` changes them too: the settings are in `org.gnome.shell.extensions.amethystora-ui.tiling`,
and the shortcuts in `org.gnome.shell.extensions.amethystora-ui.tiling.keybindings`, which
[the setup file](setup.md) keeps.

### Classic windows

If you would rather have windows that float and overlap, as on Windows and macOS, switch to the
classic layout:

```bash
ame desktop layout classic
```

It turns the tiling off, and updates leave it off. Windows then open where GNOME puts them and you drag
them by the title bar. `Super+Up` maximises and `Super+Down` restores, `Super+Left` / `Super+Right` put
a window on that half of the screen, and `Super+Shift+Left` / `Super+Shift+Right` move it to the next
monitor. `ame desktop layout tiling` brings the strip back. The first time you log in, a notification
offers the same choice, and [Control](control.md)'s **Desktop** page has it any time, as the
**Window layout** setting.

## Windows in the classic layout

What follows is for the classic layout, unless it says otherwise. `ame desktop windows` lists each of these
settings and how it is set now, and [Control](control.md)'s **Desktop** page has them under **Windows**,
**Hot corners** and **Touchpad gestures**.

### Snap layouts

![Snap layouts](media/snap-layouts.webm)

Drag a window by its title bar to the top of the screen, or press `Super+Z`, and six layouts appear:
halves, two thirds and a third, thirds, quarters, the middle, and the whole screen. Move over a part of one
and the screen shows where the window will go; let go, or click, and it goes there. With `Super+Z` the
arrow keys move between the parts and `Enter` chooses. Dragging it away gives it its old size back. The top
edge itself still maximises, and `Super` and the arrows still work. Off with `ame desktop windows snap off`.

### Switching windows

![Switch windows](media/super-tab.webm)

`Super+Tab` and `Alt+Tab` show every window in a grid, the windows themselves, live. Keep the key down and
press `Tab` to move on, or start typing: only the windows whose app or title has those letters stay, and
the grid stays open after you let go, until `Enter` switches or `Esc` leaves everything as it was. In the
tiling layout the keys switch as GNOME does. `ame desktop windows switcher off` gives GNOME's switcher back.

### Holding Super

![Hold Super](media/hold-super.webm)

Hold `Super` for a second, without another key, and a card shows the common shortcuts, as they are set on
this machine, your own changes included, with numbers on the dock: `Alt` and a number opens the app in that place.
Let go and they fade. A quick press of `Super` still opens the overview. [Control](control.md)'s Keyboard page has
every shortcut. `ame desktop windows hold-super 1500` waits a
second and a half; `off` turns it off. This works in both layouts.

### Hot corners

Each corner of the screen can do one thing when you push the pointer into it: the overview, show the
desktop, lock the screen, a quick note, or the notifications. All four do nothing until you choose, in
Control or with `ame desktop windows corner top-right overview`. A corner fires after a short push past
the edge, so passing by never sets it off. Corners next to another screen have none, and GNOME's own hot
corner, if you turn it on in Settings, keeps the top left. The corners work in both layouts; show the
desktop only in the classic one.

### Show desktop

With `ame desktop windows show-desktop on`, clicking empty wallpaper slides every window to the edges,
leaving a sliver of each. A click anywhere, or any key, brings them back; clicking a sliver also brings
that window to the front.

### Touchpad gestures

![Pinch for the apps](media/pinch-apps.webm) ![Spread for the desktop](media/spread-desktop.webm) ![Four fingers for Today](media/four-left-today.webm)

| Fingers | Does |
| --- | --- |
| Three up | GNOME's overview; again for the app grid |
| Three sideways | GNOME's next workspace |
| Pinch with three | The app grid |
| Spread three | Show the desktop; pinch to bring the windows back |
| Three down | The windows of the app you are in, fanned out from the dock |
| Four to the left | Today and the notifications; to the right closes them |

Two fingers stay the app's own. Each is a switch: `ame desktop windows gesture pinch off`. The pinch and
four fingers work in both layouts, unless the tiling's own gestures use four fingers.

### Places

Windows go back to their places for each arrangement of screens: plug a screen in and each app's windows go
where they were the last time it was there; unplug it and they return to their laptop places. Apps cannot
place themselves on Wayland, so Amethystora keeps the places, by the screens' connectors and sizes, in
`~/.local/state/amethystora/places.json`. `ame desktop windows places forget` forgets them.

### Keep a copy in view

Choose **Keep a copy in view** in a window's menu (right-click its title bar, or `Alt+Space`): the window
goes out of sight and a small live copy of it stays on top of every workspace, in a corner. Drag it to
another corner; click it to bring the real window back. Some apps stop drawing while out of sight, and the
copy then shows their last picture. This works in both layouts.

### Dimming

`ame desktop windows dim on` draws the windows behind the one you are using a little darker, so the eye goes
to the right one. A video an app draws on its own may stay bright.

### Drop-down terminal

`Super+Alt+Return` slides a terminal down from the top of the screen, over whatever is open and on any
workspace; the same key slides it away again, still running, with whatever it was doing. Pressed while the
terminal is down but another window has the keys, it gives the terminal the keys instead. It is kitty,
started the first time you press the key, with no title bar (type `exit` to close it); the next press starts
another. This works in both layouts. `ame desktop windows drop-down off` turns it off, and a terminal that was
down stays open as an ordinary window.

In the classic layout any window can join it: **Move to the scratchpad** in the window's menu (right-click
its title bar, or `Alt+Space`) puts it out of sight, and `Super+Alt+Return` then shows and hides it with the
terminal, each where it was left, on every workspace and above the rest. **Take out of the scratchpad**, in
the same menu, gives it back as it was. The tiling has its floating windows for this instead
(`Super+Alt+Esc`). `ame desktop windows scratchpad off` turns the scratchpad off.

### Keep on top

`Super+Alt+P` keeps the window you are using above every other window, and again lets it go. A window kept on
top shows it: a thin ring in the theme's accent and a pin on its top corner, which you click to let it go.
**Always on Top** in the window's menu does the same. `ame desktop windows keep-on-top off` turns the key and
the ring off; windows already on top stay so.

### See-through windows

With `ame desktop windows see-through on`, hold `Super+Alt` and scroll over a window: down to see more of what
is behind it, as far as 40 %, up to make it solid again. The window forgets it when it closes. To have an app's
windows always open see-through, give it a level: `ame desktop windows see-through-app kitty.desktop 92`, or
the terminal's own slider in [Control](control.md); `off` in place of the number takes the rule away. While
the screen is shared, a window the [share guard](#share-guard) covers is solid. `Super` and the scroll wheel
alone still change workspace.

### Window frame

With `ame desktop windows frame on`, the window you are using has a still line round it, from the theme's
warm colour through its accent to its cool one, in the classic layout (the tiling frames the window in use
its own way). A maximised window, and one kept on top, have none.

### Sizes for recording

**Size for recording**, in a window's menu, makes the window exactly 1280 × 720, 1920 × 1080, 1080 × 1080
or 1080 × 1920 and puts it in the middle of its screen, for screencasts and screenshots that line up. The
sizes are the screen's own pixels, whatever its scale; one that does not fit the screen is greyed out. In
the tiling layout it is offered for floating windows only. `ame desktop windows recording-sizes off`
takes it out of the menu.

### Reopen

Closed the wrong window? `Super+Alt+T` opens that app again, on its workspace and, in the classic layout, at
the place and size its window had; press it again for the one before, five back. Only the app is started:
what it had open is the app's own to bring back. What closed is kept in memory only and forgotten when you
log out. `ame desktop windows reopen off` turns it off and forgets.

## Window settings

The window settings GNOME keeps out of sight, in [Control](control.md)'s **Window settings** and with
`ame desktop window-settings`, which lists them:

| Setting | Command |
| --- | --- |
| The window under the pointer takes the keys | `ame desktop window-settings focus-follows-pointer on` |
| New windows open in the middle of the screen | `ame desktop window-settings centre-new on` (on as the image ships) |
| What a double, middle or right click on a title bar does | `ame desktop window-settings middle-click lower`: `toggle-maximize`, `minimize`, `lower`, `menu` or `none` |
| The key held to drag a window from anywhere in it | `ame desktop window-settings drag-key alt` (`super` as the image ships) |
| Resize with the right button, the drag key held | `ame desktop window-settings resize-right on` |
| Dialogs stay attached to their window | `ame desktop window-settings attach-dialogs on`, in the classic layout: the tiling keeps them apart |

With `Alt` as the drag key, apps no longer see a click made with `Alt` held.

## Force quit

`Super+Ctrl+Shift+Esc` lists the apps that are running, with how much of the processor and memory each
uses now, counting everything it started (a terminal's commands, a browser's helpers). An app that does
not respond comes first, marked **Not responding**. Pick one and press **Force quit**: the card asks once
more, naming it, since anything not saved in it is lost. Force quit ends only that app's own processes.
`ame desktop force-quit off` turns the key off.

When an app stops responding, the desktop asks over its window whether to **Wait** or **Force quit**.
Waiting is often enough, and is what `Esc` and `Enter` do. While the agentic features are on, **Ask the
agent why** opens the agent on that app, to find out what it is waiting for without ending it (see
[the AI agent](ai-agent.md)). `ame desktop frozen-dialog off` brings back GNOME's own question.

## Windows

| Key | Does |
| --- | --- |
| `Super+W` | Close the window |
| `Super+Backspace` | Resize with the arrow keys |
| `Shift+F11` | Full screen, keeping the title bar |
| `Super+H` | Minimise the window |
| `Super+Tab` / `Alt+Tab` | Switch between apps / between windows; type to filter (classic layout) |
| `Super+Z` | Snap layouts for the focused window (classic layout) |
| `Super+Alt+Return` | The drop-down terminal, and the scratchpad in the classic layout |
| `Super+Alt+P` | Keep the window on top, or let it go |
| `Super+Alt+T` | Reopen the app you just closed |
| `Super+Alt` and scroll | See through a window, when see-through windows are on |
| Hold `Super` | The main shortcuts, and numbers on the dock |
| `` Super+` `` | Switch between the windows of the app you are in |

The three lights on the right of every title bar minimise, maximise and close the window, amber, green and
red in every theme, and grey while the window is not the one you are using. Move the pointer over them and
each shows its sign.

## Motion

Minimize a window and it gathers into a drop of glass, which arcs down into the window's icon in the dock; a ring
ripples out of the icon as it lands. Bring the window back, from the dock, `Alt+Tab` or anywhere else, and the drop
rises from the icon and swells back into the window. An app you open from the dock grows out of its icon the same
way. Changed your mind half-way? Click the icon again and the window turns round from where it is.

Changing workspace has depth: the windows travel a whole screen, the wallpaper only a third of the way, and the
windows step back a little as they go.

```bash
ame desktop minimize shrink          # GNOME's own: the window shrinks into its icon
ame desktop minimize droplet         # back to the drop
ame desktop workspace-switch slide   # each workspace slides in whole
```

With [Reduce motion](themes.md#comfort) on (`ame desktop comfort reduce-motion on`), windows fade in place instead
and workspaces crossfade. With animations turned off in Settings, under Accessibility, nothing moves at all.

A window can also arrive and leave its own way: **Pop** grows it into place from a little smaller, **Slide** raises
it a little into place, **Fade** only fades it in; closing plays it backwards. Dialogs keep GNOME's. It is GNOME's
own until you choose one (Control's **Opening and closing**):

```bash
ame desktop window-motion pop   # or slide, fade, gnome
```

How fast all of this plays is the [animation speed](themes.md#comfort).

## Workspaces

There are always six, so `Super+3` always means the same place. A common way to use them: a browser
on 1, a terminal on 2, chat on 6. `Super+Shift+3` takes the focused window to workspace 3.
`Super+Page Up` / `Super+Page Down` go to the previous / next workspace, and
`Super+Shift+Ctrl+Left` / `Super+Shift+Ctrl+Right` move the window to the next monitor.
`Super+Ctrl+Alt+Left` / `Super+Ctrl+Alt+Right` swap the workspace you are on with the one beside it,
windows and name included.

A workspace shows its number, and after it the name of the app with most windows on it, in lighter type. Give it a
name of your own with a right click on it; **Use the app's name** in that menu goes back to the automatic one, and
`ame desktop space-names off` turns automatic names off. To move a window to another workspace, open the overview
and drag the window onto the workspace in the top bar, or onto its picture at the top of the overview.

In the overview, each workspace's picture at the top carries the same name along its bottom edge (its number when it
has none), and a workspace with a wallpaper of its own (the tiling's **Workspaces** settings) shows that wallpaper
there, under its windows.

### Saved sets

Right-click a workspace in the top bar and choose **Save this workspace**: its apps and where their windows are become a
set, named after the workspace. The set is in the same menu under **Sets**; click it to open it again. Its apps start and
each window goes back to its place as it appears, on the workspace you are on if it is empty, or else on the next empty
one. Windows of those apps already there are put in place rather than started twice, and nothing is closed. An app that
can have only one window brings that window over from wherever it is.

A set keeps the apps and their places, not what was open in them: apps that reopen their own documents (most browsers
and editors do) open them again; others start empty, and a browser that brings back its own windows may open one more
than the set had. While a set opens, a window you open on another workspace stays where you opened it: only the windows
that appear on the set's workspace, or while you are on it, are put in place. With the tiling on, the windows come back
in the same order and widths. A set keeps at most sixteen windows, the ones you used last. Up to twelve sets are kept,
in `~/.local/state/amethystora/sets.json`; the cross on a set forgets it. `ame desktop sets off` takes Sets out of the
menu and keeps what was saved.

## Extensions

These GNOME Shell extensions come with the image:

| Extension | What it does |
| --- | --- |
| Amethystora Clipboard | The [clipboard history](#the-clipboard) on `Super+Shift+V`, sealed: a copy of Clipboard Indicator |
| Amethystora UI | Amethystora's own desktop, written for this image: [tiling windows](#tiling-windows) side by side on a scrolling strip, the dock, the glass (Frost, Prism or Clear, see [Glass](themes.md#glass)) behind the panel, the overview, menus and notifications (windows you are not using are made slightly see-through), the workspaces, the meters and the tray icons in the top bar, a phone icon there while a [paired phone](hardware.md#phones) is connected, the search on `Super+Space` app search that forgives typos, drawing on a screenshot and [copying the text out of it](#copying-text-from-the-screen), and uninstalling an app from its menu, quicker animations, no popup when you change workspace, and no Activities button; the [live pill](#the-live-pill), [privacy dots](#privacy-dots), [quiet notifications](#quiet) and the volume pill |
| Logo menu | The Amethystora logo at the top left and [its menu](#the-top-bar) of the system's tools |
| Amethystora Transitions | The animation that reveals a new theme or wallpaper ([Transitions](themes.md#transitions)) |
| Amethystora Widgets | Your own [widgets](widgets.md) in the top bar; nothing until you add one |

Turn any of them off, or change their settings, in **Extensions** (Extension Manager is for finding
new ones on extensions.gnome.org, which has none of these). For these five, **See Details** in Extension
Manager 0.6.5 says "An Error Occurred: No extension details available", because it asks only that site:
nothing is wrong with them. Each one's description and version are in Extension Manager's row, under the
arrow, and in **Extensions**, under the three dots. One thing to know: after
every image update the image's own extensions are switched back on, so that one turned off by
accident, or by a crash, does not stay off for good. Turning one off lasts until the next update. The
[classic layout](#classic-windows) is not an extension turned off, so it stays as you set it.

## Tools at hand

### Pick a colour

`Super+Alt+C` holds the screen still and puts a loupe under the pointer: the pixels around it, magnified, and the
colour under it named. Click to copy the colour as `#9148F0`, `Shift`+click as `rgb(145, 72, 240)` and
`Ctrl`+click as `hsl(266, 85%, 61%)`; `Esc`, a right click or the key again closes it with nothing copied. The
colour goes on the clipboard (and so into its history), and the last eight you picked are in the search: type
`colour:`. Nothing of the screen is kept once the loupe closes, and it does not open while the screen is shared.

### Mute the microphone

`Super+Alt+M` mutes the microphone for every app at once, in a call or not: it is the input's own mute, the one the
microphone slider in quick settings sets. While it is muted, **Mic off** stays in the top bar; its menu turns the
microphone back on, and so do the key again and the slider. A microphone you plug in meanwhile (a headset) is muted
too. The [privacy dots](#privacy-dots) list has the same **Mute** button beside the apps using the microphone.

### Share this Wi-Fi

In quick settings, the Wi-Fi menu has **Share this Wi-Fi** while you are on a network a phone can join with its
password alone. Click it and the menu shows a QR code: a guest's phone camera joins with it, with no password read
aloud or typed. The password is read only then, may need yours to read, and goes when the menu closes. It is not
offered for a network that needs a user name (802.1X) or for the hotspot this computer makes, and not shown while the
screen is shared or recorded.

## Files

`Super+Shift+F` opens a Files window and `Super+E` your home folder. Folders open on a grid, where
the icon theme's folders show as the artwork they are.

### Folder colours

Select one or more folders in Files, right-click, and pick **Scripts › Folder colour ›** Amethyst, Sky, Jade, Amber,
Coral or Slate (Files lists them in alphabetical order). The folder takes that colour at once in Files, and a stack
of it in the dock takes it as you open the stack or bring the pointer to the dock. The colour is drawn by your icon
pack, so it follows when you change the pack. **No colour** takes
it away, and gives Documents, Music, Pictures and the other special folders their emblem back. A picture you set as a
folder's icon yourself is left alone, and files in the selection are left as they are. Stars are Files' own: the star beside a
file in the list view, and **Starred** in its sidebar.

### Back in time

Right-click a folder (or a file) in your home folder and pick **Scripts › Back in time**: [Backups](security.md#backups)
opens on that folder's timeline, where you can see it as it was and put back what is gone. Files shows its Scripts menu
only for something selected: for the folder you are in, right-click it in the folder above, or use **Open With ›
Backups**. Folders outside your home folder are not in the snapshots or the backup, and the menu says so.

### Earlier versions

Right-click a file in your home folder and pick **Scripts › Earlier versions**: [Backups](security.md#backups) opens on
that file's own versions, each with when it was made. **Look** opens one read-only, beside the file you have, and **Keep
this one too** puts it next to your file with its date in its name; your file is never touched. It shows what the hourly
snapshots and the backup hold, those on this machine at once and those in the backup when it answers, and it says when either
is off. A file in the [private folder](security.md#a-private-folder-that-locks-with-the-screen) is in neither. `ame desktop
earlier-versions off`, or **Earlier versions in Files** in [Control](control.md), takes the entry away.

### A file nothing opens

Open a kind of file nothing here opens and a card says what it is, in plain words, and which app on Flathub opens it, with
**Install and open**. Each app says whether Flathub has checked who made it: any app can say it opens a kind of file, and the one
you install is started on this file with whatever it asks for. The file stays on this computer, and only its type is looked up, in
the list of apps Flatpak keeps here, so nothing is sent until you press the button. A kind no app on Flathub claims, or a file that does not say what it is, is
said too, and **Open With** in Files can still try any app on it. An app you install takes those files from then on, without
the card. `ame desktop open-anything off`, or **A file nothing opens says what it is** in [Control](control.md), brings back
GNOME's own message for your account.

### Look at a file with Space

Select a file in Files and press `Space` for a preview: pictures, PDFs, text and video. The arrow keys walk the folder, the
button in the preview opens the file in its app, and `Space` or `Esc` closes it. It is GNOME's previewer, one of the apps that
[install the first time the machine is online](software.md#apps-bazaar-and-flathub), so it works from then on. It runs only
while it shows and keeps nothing, and it can read your home folder only, so a file on a USB stick does not preview.

### Copy without hidden data

Select one or more files and pick **Scripts › Copy without hidden data**: beside each, a copy named
`NAME (clean).EXT` without what the file carries besides its content, and a notification says what was
taken out (*Removed: place, camera, time*). The original is never changed. It is made by mat2:

- **Photos and pictures** (JPEG, PNG, TIFF, WebP, HEIC, GIF, SVG): where they were taken, the camera and
  lens, the times, the author, the program and comments. The picture is saved anew.
- **PDF**: the author, title, subject, keywords, the programs that made it, its dates and its XMP data. Its
  text stays selectable and its links work; comments, form fields and files attached inside may stay.
- **Office documents** (LibreOffice's and Microsoft's, and EPUB): the author, who saved it last, how many
  times and for how long it was edited, the program and the dates. A document with a part mat2 does not
  know is not cleaned at all, rather than half.
- **Music and sound** (MP3, FLAC, Ogg, WAV, AIFF): tags, comments and embedded pictures. **Video** (MP4,
  AVI, WMV): its metadata.
- **Archives** (zip, tar): their own details and each file's inside.

What the content itself shows or says stays: a face, a street sign, words in a picture, a name typed in a
document, and the file's own name. A file of another kind is left as it was, and the notification says so.

### What is using this?

When a stick will not eject or a file will not go, select it (or the stick's folder) and pick **Scripts › What is
using this?**: a window lists the programs of yours that hold it, and what each holds (*its folder is open here*,
*report.txt is open*), each with **Quit**. Only your own programs are listed: never another account's. GNOME's own
**Volume is busy** dialog gains **Show what is using it**, and so does the dock's notice when a drive cannot be
ejected.

### Convert and shrink

Offered and off: `ame desktop files-convert on` (or **Convert pictures in Files** in [Control](control.md)) adds
**Scripts › Convert** to Files, with **As PNG**, **As JPEG**, **As WebP**, **Half the size**, **1920 pixels wide** and
**Into one PDF**. Each writes a new file beside the picture, never over one: `holiday.png`, `holiday (half
size).jpg`, `holiday (1920 px).jpg`, and for several pictures one PDF named after the first, a page each. A picture
is turned upright first, and its metadata (where a photo was taken among it) is not carried over. A JPEG is saved
over white where the picture was transparent. Pictures Files cannot show cannot be converted either: an iPhone's HEIC
needs a decoder Fedora does not ship. `ame desktop files-convert off` takes the menu away.

These are scripts in `~/.local/share/nautilus/scripts`. The first five are written once for each account: delete one
and it stays gone. Convert is there exactly while it is on.

### New documents

Right-click inside a folder in Files and pick **New Document**: an **Empty text**, a **Markdown note**, a **Shell
script** or a **Python script**, with its first line written, ready to be named. They are the files in your Templates
folder, written once when it was empty, in your language; change them, add your own or delete them, and Files offers
what is there. A script made this way is a text file until you make it executable yourself.

### Downloads keeps itself tidy

Off until you turn it on, in Control's **Apps** page or with `ame desktop tidy-downloads on`. Once a day, installers in
your Downloads folder (`.exe`, `.msi`, `.deb`, `.rpm`, `.AppImage`, `.flatpakref` and the like) that were last opened
more than a week ago go to the [trash](#the-trash), and other files last opened and changed more than a month ago go into
a folder for their month, such as `2026-09`. The newest stay where they are. Only the files in Downloads itself are
moved, never a folder, a hidden file or one still downloading, and never over another file. Nothing is deleted: the
trash keeps what it is given for 30 days. A notice says what moved, with **Undo**, and `ame desktop tidy-downloads undo`
puts the last tidying back later.

```bash
ame desktop tidy-downloads on          # off, undo, or now to tidy at once
```

## The trash

The trash empties itself of what you deleted more than 30 days ago. GNOME checks it every hour while
you are logged in, and the first check comes an hour after you log in. It does the same to the trash of
a USB disk or other removable drive that is plugged in at the time. A file emptied from the trash is
still in the [hourly snapshots and the backup](security.md#backups), where you have them on.

Choose another age, or never, in [Control](control.md)'s **Empty the trash**, in GNOME Settings under
**Privacy**, or in a terminal:

```bash
ame desktop privacy trash 7     # what was deleted more than a week ago
ame desktop privacy trash off   # only when you empty it yourself
```

**Empty Trash** in Files empties all of it at once, whatever the age.
