# Comfort and reading

Switches for a calmer, clearer desktop, and help with seeing and reading what is on it.

## Comfort

Control's **Appearance** page has a **Comfort** group; `ame desktop comfort` alone shows how each is set.

- **Reduce transparency** draws every glass surface in the theme's solid colours. Nothing is blurred and the
  glass's pictures are let go, so it also costs less: `ame desktop comfort reduce-transparency on`, and `off`
  to go back.
- **Reduce motion** makes Amethystora's animations that travel (the dock's, a window going down to the dock) fade in
  place instead: `ame desktop comfort reduce-motion on`. It is GNOME's own Reduced Motion, which apps follow too.
  Turning **Animations** off in Settings, Accessibility, counts as reduce motion as well, and so does a screen drawn
  without a graphics card's help (some virtual machines) or a remote desktop, where GNOME turns animations off by
  itself whatever the switch shows.
- **Increase contrast** is GNOME's High Contrast (Settings, Accessibility): the glass turns solid, every surface gets
  a strong outline in the text colour, dimmed text is shown plain and controls are outlined. libadwaita apps follow
  (see [Colours in applications](themes.md#colours-in-applications)): `ame desktop comfort increase-contrast on`.
- **The power saver** (quick settings' power menu) makes the glass solid and every animation take half its time while
  it is on. Nothing here turns it on by itself.
- **Animation speed**: snappy, normal (the image's), relaxed, or off, without touching GNOME's own Animations switch:
  `ame desktop animation-speed relaxed`. Reduce motion still turns travel into fades, whatever the speed.
- **Text size**, 90 to 150 percent, apart from the screen's scale, which stays whole: `ame desktop text-size 115`. It
  is GNOME's text scaling: the shell and Amethystora's apps follow at once, other apps at their next start.

### The light desktop

The look without the costly parts, for a computer without a graphics card: the glass is solid, motion is short and
simple, and the dock shows no live window previews. A computer that draws without a graphics driver, or has less than
8 GB of memory, is offered it once in a notification after the first login (and again if its graphics change); either
answer is remembered. Control's **Light desktop** switch, or:

```bash
ame desktop light-desktop on    # off to go back
```

## Seeing and reading

GNOME has a screen reader, zoom and large text in Settings, **Accessibility**. Amethystora adds the ones below, in
Control's **Appearance** page, group **Seeing and reading**, and with `ame desktop access` (alone, it shows how each is
set). All are off until you choose them.

### Colour filters

A filter over the whole screen, every app and the desktop itself:

- **Red (protan)** and **Green (deutan)** pull apart the reds and greens that an eye with a red or a green weakness
  mixes up, by moving what that eye misses into the colours it sees well: `ame desktop access filter deutan`.
- **Blue-yellow** does the same for blues and yellows: `ame desktop access filter tritan`.
- **Grey** draws everything in grey, for fewer distractions: `ame desktop access filter grey`.

The strength is **Half**, **¾** or **Full** (`ame desktop access strength 75`; any number from 25 to 100). While a
filter is chosen, quick settings has a **Colour filter** tile to turn it off and on and pick another filter or
strength. `ame desktop access filter none` takes the filter and the tile away.

#### What the filter covers

- Zoom's magnified picture is filtered; the pointer is not (except in some virtual machines, where the desktop draws it).
- **Screenshots are saved without the filter**, in the apps' own colours, so they look right to everyone; the
  screenshot tool on screen is filtered.
- **Recordings and screen shares may show the apps' own colours**: for a whole screen it depends on the program that
  makes it; an area or a window never shows the filter.
- The lock screen is not filtered, nor a screen in HDR, which the filter does not work with yet (it says so the first
  time).
- While a filter is on, a fullscreen game is drawn through the desktop like any window, and each screen is copied once
  more every frame, which costs a little speed and battery.

### The reading ruler

A band that follows the pointer, for anyone who loses their line: dyslexia, tired eyes, long pages. **Super+Ctrl+R**
hides it and shows it again.

![The reading ruler](media/reading-ruler.webm)

It lies across the window you are using while the pointer is over it, else across the screen, and is as tall as a
line of text (taller when you make text larger in Settings).

- **Band**: a highlighter's yellow band: `ame desktop access ruler band`.
- **Dim the rest**: the window (or screen) above and below the line is veiled: `ame desktop access ruler dim`.
- **Underline**: a line in the theme's accent under the line you read: `ame desktop access ruler underline`.

`ame desktop access ruler off` turns it off, and its shortcut with it. The ruler shows in screenshots.

### Reading aloud

With `ame desktop access read-aloud on`, select text in any app and press **Super+Ctrl+S**: it is read aloud on this
computer with the screen reader's voice. Nothing is sent anywhere, and no screen reader needs to be on.

The live pill says **Reading aloud** with the speed, which steps through 1×, 1.5×, 2× and 0.75× from the next sentence
(roughly), and **Stop**; **Super+Ctrl+S** again stops too, and also cuts short the sentence the screen reader is saying
at that moment. A long selection is read up to about twenty thousand characters. The voice is eSpeak NG through Speech
Dispatcher, as Orca's is; other voices Speech Dispatcher knows can be set in its own settings.

### Click with the keyboard

With `ame desktop access letters on` (or **Click with the keyboard** in [Control](control.md)), press **Super+Alt+K**
in any window: a small letter appears on each button, link, box, tab and menu item. Type a control's letters to click
it (a text field gets the cursor instead). `Backspace` takes back a letter; `Escape`, a click or a letter that matches
nothing takes them away. With many controls each has two letters.

The letters come from the window's accessibility tree, the one screen readers use, and only while they show: nothing
is read before **Super+Alt+K** and nothing is kept after. GTK apps (Settings, Files, Amethystora's own) say the most.
Firefox and Chromium or Electron apps fill in their tree only while a screen reader runs, so they may show no letters.
`ame desktop access letters off` turns it off, and its shortcut with it.
