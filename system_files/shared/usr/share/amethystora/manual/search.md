# Finding things

`Super+Space` searches apps, files, settings and answers worked out on this machine; `Super` alone opens the overview,
and `Super+A` every installed app. Nothing you type is sent anywhere.

## The search box

`Super+Space` opens a search box near the top of the screen, over your windows. Type, and `Enter` opens the first
result; `Ctrl+1` to `Ctrl+9` open the result with that number, shown at the end of each of Amethystora's rows.
`Escape`, the same keys again or a click elsewhere closes it. Right-click a result for its menu, or drag an app onto the
dock to pin it.

![Search anything](media/super-space.webm)

`Super` alone opens the overview: every open window, with search at the top. Both searches forgive typos, missing
letters and initials in app names (`calxu` finds Calculator, `mc` Mission Center), and find Power Off, Restart and the
other actions too.

## Answers from this machine

Besides apps, files and GNOME's settings, both searches answer from this machine, and nothing is sent anywhere.

### Sums and conversions

As you type, in your language's numbers: `1284 * 0.18` gives 231.12, `200 + 10%` gives 220, `3.5 km in miles` gives
2.1748 mi and `21 °C in °F` gives 69.8 °F. `Enter` copies the answer. It converts lengths (mm, cm, m, km, in, ft, yd,
mi, nautical miles), weights (mg, g, kg, t, oz, lb, stone), temperatures (°C, °F, K), data (bits and bytes, kB to PB,
KiB to PiB) and time (ms to weeks). No currencies: their rates would have to come from the internet.

### The time in other places

`time in Tokyo` gives the time there now, the day, and how far ahead or behind you it is; `15:00 Paris in Tokyo` gives
23:00 in Tokyo, the same day, and `15:00 in Tokyo` starts from your own time. A place is any city or country the world
clocks know, in your language or English, a time zone such as `Europe/Paris`, or `UTC`, `UTC+2`. Times follow your
clock (24 or 12 hours), `Enter` copies the answer, and it is worked out from the computer's own time-zone data.

### Reminders

`remind me at 17:00 to call the garage`, `remind me tomorrow at 9:00 to pay the rent`, `remind me in 20 minutes to take
the pizza out`. The row shows the task and when; `Enter` makes it a task in [Notes](notes.md#reminders)' inbox with a
reminder at that time, and the row under it does the same a day later. When Notes is locked, its quick note opens with
the task in it and asks for the passphrase first. The words are English, as Notes' task box reads them, and a reminder
needs a time.

### Files by what you remember

`pdf from last week`, `big videos`, `photos yesterday`, `lease from march`, `invoice over 2 mb`. A small set of rules
reads kinds of file, dates and sizes from what you type, in your language or English, and asks the file index GNOME
already keeps; other words have to be in the file's name.

![Files by what you remember](media/search-remembered.webm)

It takes two words or more, so a word alone is left to the apps. There is no model and no agent, it reads no file, and
it keeps nothing; words it cannot read just search as usual. `ame desktop search remembered off` turns it off.

### Settings, windows and more

- **Do it**: [Control](control.md)'s settings, by their words. `Enter` on a row such as *Phone pairing: On* changes it
  at once; one that asks something or needs a password opens a terminal. *Control › …* opens it in Control.
- **Open windows**, by title or app, on any workspace: `Enter` switches to it.
- **The words you already know**: type what you would have called it, such as `recycle bin`, `task manager`,
  `control panel` or `print screen`, and a row takes you to the thing that does that here. A short table of everyday
  words, in your language and English: `ame desktop search old-words off` turns it off.
- **The manual**: the pages that answer what you typed, by meaning as well as by words.
- **Clipboard history**, only when you type `clip:` first, with words after it to narrow it down: `clip: ssh`. `Enter`
  pastes the item where you were typing. It is the history [the clipboard](clipboard.md#the-clipboard) already keeps;
  the search stores nothing new.
- **The agent**, only while the [agentic features](ai-agent.md) are on: the last row, *Ask the agent: …*, opens your
  agent in a terminal with what you typed as its first question. Nothing starts before `Enter`, and the agent asks
  before it changes anything.
- **Glance**: with the keyboard on a file Files found (`↓` to it), `Space` shows it without opening it. `↑` and `↓` go
  to the other files found; `Space` again closes it, and the search is still there.

## Offered, and off

Three more answers are off until you turn them on:

- **Bookmarks**: Firefox's bookmarks whose name, address or folder hold what you typed; `Enter` opens one in your
  browser. They are read from a copy of Firefox's bookmarks file, deleted at once; Firefox's history is never read, and
  Firefox itself is left exactly as it is. `ame desktop search bookmarks on`.
- **Notes**: the titles of your [notes and tasks](notes.md) that hold what you typed, only while Notes is open and
  unlocked; `Enter` opens one in Notes. A note's text is never searched, and locked or closed, Notes says nothing.
  `ame desktop search notes on`.
- **Files by meaning**: files in your Documents folder found by what they are about, even when no word you typed is in
  them, with two words or more: *the letter about the tax refund* finds `refund-2025.pdf`. It uses the small model the
  [AI agent's](ai-agent.md) index already has, on the text GNOME's indexer read from the files, and works out what each
  is about once a day, on mains power only, at low priority. It keeps the files' names and numbers in
  `~/.local/state/amethystora/file-meaning`, never their words, and sends nothing. `ame desktop search meaning on`;
  off deletes what it kept.

Each answer can be turned on or off in Control's **Search** group or in a terminal:

```bash
ame desktop search                     # each, and whether it is on
ame desktop search answers off         # answers, times, reminders, timers, settings, manual, windows, clipboard, agent,
                                       # glance, old-words, remembered, bookmarks, notes or meaning
```

## The app grid

`Super+A`, the app grid button on the dock, or a pinch with three fingers on the touchpad opens it.

- **Recently used**: the apps you use most, in a row at the top, most used first, as many as fit (eight at most). At
  the very first login it holds only the apps that opened by themselves, the Manual and Control; an account that has
  never opened an app has no such row.
- **All apps**: the folders, then the apps that stay loose.

The folders fill themselves from what each app says it is: **Office**, **Graphics**, **Games**, **System**,
**Development** and **Utilities**, named in your language. An app that is two of these is in both. Security, Updates,
Control, Backups, the Manual and Phone stay loose, and so do Steam and Lutris.

![Pinch for the apps](media/pinch-apps.webm) ![Folders fill themselves](media/app-grid-folders.webm)

The folders are GNOME's own: rename one, take an app out or move the icons, and that is kept for you; folders you had
changed before stay as they were. A second page starts only once the first is full: the icons keep the order you gave
them, and a gap on a page closes.

```bash
ame desktop app-grid recent off        # no row of recent apps; on brings it back
ame desktop app-grid folders reset     # the folders as the image has them, forgetting your changes
```

### Hide, rename or change an app

Right-click an app in the grid, a folder or the search:

- **Rename…** changes the name under its icon, and only that: its launcher stays as it is, and the search finds it by
  both names.
- **Change icon…** shows the app as each installed icon pack draws it; **A picture of your own…** takes a PNG, JPEG,
  SVG or WebP and makes it an icon in `~/.local/share/amethystora/app-icons`; **Reset** gives its own icon back. An app
  whose icon is its maker's logo has no other pack's drawing to choose: no pack draws another maker's logo.
- **Hide from the grid and search** takes it out of both, with **Undo** in the notice. A hidden app still opens from a
  terminal and from **Open With**, and stays in the dock if pinned; shown again, it comes back at the end of the grid.
- **What can it reach?**, on a Flatpak app, opens Security's Apps page on it: what it can reach beyond its sandbox, and
  what Security can change. Nothing in the menu changes a permission.

The new name and icon show in the grid, the dock, the search and the window switcher. Control's **Apps** page lists
every app you hid, renamed or gave another icon, each with the way back. Only the app grid's own settings are written,
never a launcher of your own, which the [security watcher](security.md) would read as something planted.

```bash
ame desktop app hide org.gnome.Calculator.desktop                    # show brings it back
ame desktop app rename org.gnome.Calculator.desktop "Sums"           # without a name, its own again
ame desktop app icon org.gnome.Calculator.desktop amethystora-neon   # reset, or picture FILE
```

### The app grid in order

The grid keeps the icons where you put them, as GNOME does. Control's **The app grid's order** (Desktop page) sorts it
instead: **A to Z**; **By use**, most used first, from GNOME's own record of use kept on this machine; or **By kind**,
from what each app says it is (work, pictures and sound, development, games, then the system's). Your folders stay
first, as you left them, and **Where you put them** gives your own order back.

```bash
ame desktop app-grid order name        # custom, name, use or kind
```

## Your photos, by what is in them

Describe a photo and find it: in the search, type `photo:` and a few words, such as `photo: the dog at the beach`, and
the pictures of your Pictures folder that look like that come up. Off until you choose it, with
`ame desktop search photos on` or **Your photos, by what is in them** in [Control](control.md).

- It downloads a model once (about 150 MiB, MIT licence, checked against the SHA-256 in the image), which reads each
  photo **once, on this computer**, only on mains power with nobody touching it for 30 seconds, and stops the moment
  someone does. Photos are read through the same sandboxed picture readers as the rest of the system.
- What it works out (a short list of numbers and the photo's name) is kept **sealed in the [Vault](vault.md)**, so it
  can be searched only while the vault is open; with it closed, the search says so.
- Nothing in `Private`, in a folder starting with a dot, or behind a link is read, and no photo leaves this computer.

`ame desktop search photos off` stops it and deletes the model, the index and the vault's key for it.
