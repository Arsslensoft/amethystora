# Themes

A theme is one palette, and everything is painted from it. Switch themes and the whole desktop
changes together: GNOME's light or dark style and accent colour, the terminal, the shell prompt, the
top bar, the dock, the wallpaper, VSCodium, and this manual. Each theme also has its own
[style](#styles), [font](#fonts), [icons](#icons) and [pointer](#pointers), and you can keep any of them whatever the theme (a
pointer you choose yourself in Settings stays too). A light theme gives the dock a faint dark
tint, so its icons still stand out on a pale wallpaper. The colours reach the applications too, at a contrast
that reads: see [Colours in applications](#colours-in-applications).

## Switching

[Show me](guide:change-the-look)

| Key | Does |
| --- | --- |
| `Super+Ctrl+Shift+Space` | Pick a theme |
| `Super+Ctrl+D` | Flip between the light and dark version of the theme |
| `Super+Ctrl+Space` | Next wallpaper of the current theme |

The same from a terminal:

```bash
ame desktop theme                   # pick one from a list, or name it: ame desktop theme Nord
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
amethystora-theme reload            # apply the current one again, after editing its files
```

Or open [Control](control.md) (`Super+Ctrl+Shift+C`), whose **Appearance** page shows every theme in
its own colours, and the current theme's wallpapers. Amethystora's own apps take on the theme's colours
as it changes.

## The themes

| Theme | Name to use | Style |
| --- | --- | --- |
| Amethystora | `amethystora` | Dark, the default |
| Amethystora Light | `amethystora-light` | Light |
| Catppuccin Mocha | `catppuccin` | Dark |
| Catppuccin Latte | `catppuccin-latte` | Light |
| Everforest | `everforest` | Dark |
| Gruvbox | `gruvbox` | Dark |
| Matte Black | `matte-black` | Dark |
| Nord | `nord` | Dark |
| Rosé Pine Dawn | `rose-pine-dawn` | Light |
| Tokyo Night | `tokyo-night` | Dark |

Every window has the three coloured lights in its corner. The two Amethystora themes also bring the
Amethystora GTK theme for the GTK 3 applications that draw with a theme of their own; the others use adw-gtk3,
GNOME's own look, in the theme's colours.
`Super+Ctrl+D` flips a theme to its partner (Catppuccin Mocha and Latte, Amethystora and Amethystora
Light); a theme without a partner flips to Amethystora in the other mode.

## Colours in applications

The theme's colours reach every application, not only the ones that are Amethystora's own:

- **GNOME's and libadwaita's applications** (Files, Settings, Text Editor, Calendar and most of what a
  Flatpak installs) are drawn in the theme's surfaces, text, accent and status colours, in place of Adwaita's
  grey. The accent is the theme's own, not the nearest of GNOME's nine, and a text colour on it, a link, a
  dimmed label, an error and the line round a field are each moved in lightness until they read: 4.5 to 1 for
  text and 3 to 1 for a line, on every surface they are drawn on. The hue stays the theme's.
- **GTK 3 applications** that draw with adw-gtk3 take the same palette.
- **KDE and Qt applications** take it from `~/.config/kdeglobals` (a Flatpak of KDE's lets itself at it).
  A `kdeglobals` with colours of your own, which this did not write, is left as it is.
- **VSCodium**, on the developer image, takes the editor, sidebars, status bar, terminal and syntax colours,
  merged into your settings over its own Default Dark Modern or Default Light Modern. A colour of yours that
  the theme does not name stays.
- **The terminal, the prompt, btop and the pointer** follow too: Halo on a dark theme, Ink on a light one (see
  [Pointers](#pointers)).

While High Contrast is on (Settings, Accessibility), libadwaita applications keep their own high-contrast
colours and the palette steps aside, and comes back when it is switched off. An application that is open
takes a new theme's colours, and a new style's corners, the next time it starts; GNOME Shell, Amethystora's own
apps and the terminal change at once.

## Wallpapers

Every theme comes with three wallpapers of made-up places drawn in its own colours, and switching to
a theme puts its first one on the desktop: a neon city in the rain for Tokyo Night, an aurora over a
mountain lake for Nord, misty pines for Everforest, a desert sunset for Gruvbox, peaks under a
pastel moon for Catppuccin, an eclipse for Matte Black, and so on. `Super+Ctrl+Space` cycles
through them. The two Amethystora themes open on a cut stone's facets, and also have the crystal
field and a night sky of violet smoke; GNOME's Settings offers those under Appearance.

To add your own, put pictures (JPG, PNG, WebP or SVG) in a folder named after the theme. They come
after the theme's own pictures:

```bash
mkdir -p ~/.config/amethystora/backgrounds/amethystora
cp ~/Pictures/mountains.jpg ~/.config/amethystora/backgrounds/amethystora/
```

`amethystora-theme bg list` shows what the current theme has to cycle through, and
`ame desktop background <picture>` sets one of those directly; without a picture it shows the next, as
`Super+Ctrl+Space` does.

### Daylight

A theme's first wallpaper follows the time of day: the same place at **dawn**, by **day**, at **dusk** and at
**night**, each drawn from the theme's own colours. Dawn is fully shown at 06:30, day at 12:00, dusk at 18:45 and
night at 22:00, and GNOME fades from one to the next by itself over the three quarters of an hour to an hour and a half
before each. The lock screen shows the same picture. Only the wallpaper changes: the theme stays light or dark. Every
theme has four pictures of its own. The Amethystora theme's day is still a dark amethyst; the pale facets are Amethystora
Light's, which stays lilac through a peach dawn, a rose dusk and a pale moonlit night.

When Night Light is on with hours of your own (Settings, Displays, Night Light, Manual Schedule), dawn comes when Night
Light ends and dusk when it starts, and day and night follow from them. With Night Light's sunset schedule, which
would need your location, the times above are kept. On the day the clocks change for summer or winter time, the
pictures come an hour early or late until the next day.

Daylight is on unless you turn it off, in Control (Appearance, **Daylight wallpapers**) or with
`ame desktop daylight off`; the first wallpaper then stays as the theme was drawn. It only ever shows the theme's own
first picture: a picture of your own, set in GNOME's Settings or with `ame desktop background`, is never replaced.
`Super+Ctrl+Space` goes on to the theme's second picture as before, and round again to Daylight.

### Living wallpapers

A living wallpaper moves slowly over your wallpaper, drawn in the theme's colours: **Aurora** (bands of light swaying
across the sky), **Drifting facets**, **Rain on glass** or **Slow dusk**. It moves only while you can see it: it is not
drawn at all while a window fills the screen, and it holds still in the overview, on battery, in the power saver, while a
game is in front (Games) and with reduced motion. While it moves, each frame costs a little power, which is why it is off
until you pick one, in Control (Appearance, **Living wallpaper**) or in a terminal:

    ame desktop living-wallpaper aurora     # or facets, rain, dusk; off for the wallpaper as it is

A workspace with a wallpaper of its own shows that wallpaper instead, and the glass is still cut from the wallpaper
under it.

## Accents

Any theme can take one of twelve accents in place of its own: Amethyst, Iris, Sky, Lagoon, Jade, Moss, Amber, Ember,
Coral, Rose, Orchid and Slate. The accent is the colour of a primary button, a switch that is on, a checked box, a
selected row, the workspace you are on in the top bar, a slider and the focus ring, in the shell, the applications,
the terminal's prompt and Amethystora's own apps. Every one of them is checked against every surface of every theme,
light or dark, so that the label on it and the text drawn in it read at the contrast WCAG asks.

Choose one in Control (Appearance, **Accent**), or in a terminal:

```bash
ame desktop accent jade
```

**Follow the theme** (`ame desktop accent theme`) puts the theme's own back. The accent is kept from theme to theme.
Applications that are open take it the next time they start; the shell and Amethystora's apps change at once. A chosen
accent wins over [colours from your wallpaper](#colours-from-your-wallpaper).

## Colours from your wallpaper

Off unless you choose it: the desktop's ground, the tint of its text, the selection and the accent worked out once from
the wallpaper, in one of three ways:

- **Vivid**: the strongest hue in the picture becomes the accent, and the ground is tinted with the hue that covers
  most of it.
- **Soft**: the same hues, quieter.
- **Mono**: greys tinted with the picture's main hue; the accent stays the theme's.

It works with every theme: a light theme stays light and a dark one dark, and the theme's fonts, style, icons, pointer
and terminal colours stay as they are. The colours are worked out again when the wallpaper or the theme changes, from
the picture of the theme's own time when [Daylight](#daylight) shows it, so they do not change through the day. A
wallpaper that is not a picture (a plain colour) leaves the theme's own colours.

Choose a way in Control (Appearance, **Colours from the wallpaper**), or with `ame desktop colours soft` (`vivid`,
`mono`, or `off` for the theme's own).

## An accent per workspace

Off unless you turn it on: each workspace has an accent of its own, and moving to another workspace changes the
accent of the shell and of Amethystora's apps once the workspace has slid into place. Other applications read the
accent when they start, so they keep the theme's. The first workspace follows the theme, and the others are Sky,
Coral, Jade, Amber and Orchid until you choose.

Turn it on in Control (Appearance, **An accent per workspace**) or with `ame desktop accent-per-workspace on`. To
choose the accent of the workspace you are on, right-click a workspace's label in the top bar and open **Accent**; for
any workspace, type `ame desktop accent-per-workspace 3 coral`. Each change of workspace restyles the shell once, not while it slides.

## Transitions

A new theme or wallpaper does not simply appear. The screen holds still for a moment while everything
changes underneath it, then the new desktop opens out in a circle from the pointer, with a glow in the
theme's accent colour along the edge. It works the same whichever way you make the change: the keys
above, GNOME's Settings, or the Dark Style button in the quick settings.

```bash
ame desktop transition         # pick one from a list
ame desktop transition wave    # or name it
```

| Transition | Looks like |
| --- | --- |
| `grow` | A circle opens from the pointer. The default |
| `outer` | The new desktop closes in on the pointer from the edges |
| `wipe` | A soft edge sweeps across at an angle |
| `wave` | The same, with a wavy edge |
| `fade` | A plain crossfade |
| `random` | One of the first four, at any angle |
| `none` | No animation: the wallpaper fades as GNOME fades it |

A transition takes 1.2 seconds. To change that, give it in milliseconds after the transition, from 200
to 5000, or move the slider on Control's **Appearance** page:

```bash
ame desktop transition grow 800
```

There are no transitions while animations are turned off in Settings, under Accessibility. They come
from the Amethystora Transitions extension, which Extension Manager can turn off.

## Styles

Each theme comes with a style: the shape the whole desktop is drawn in, from the top bar to the
menus, quick settings, notifications, password prompts and other dialogs, volume and brightness
alerts, buttons, switches and entries, the dock, and the applications' windows.

| Style | Looks like | The theme it comes with |
| --- | --- | --- |
| `islands` | A floating pill for each part of the top bar, the workspaces as dots, soft corners everywhere | Amethystora Light, Catppuccin Latte, Nord |
| `floating` | One rounded top bar held off the edge of the screen | Amethystora, Everforest |
| `hairline` | A thin, solid top bar, crisp corners and fine lines | Matte Black |
| `capsule` | The clock in a dark capsule, the rest of the top bar bare over the wallpaper, everything else a pill | Rosé Pine Dawn |
| `terminal` | Monospace, square corners and lined edges, like a terminal | Gruvbox |
| `segments` | The top bar in blocks of the theme's colours | Catppuccin Mocha |
| `neon` | Dark glass outlined and glowing in the theme's accent colour | Tokyo Night |

In the applications every control is rounded by the style: buttons, entries, tabs, menu rows, switches,
cards and lists, popovers, dialogs, tooltips and toasts, and the sliders' troughs. A group of buttons or fields
is rounded at the ends of the group and straight between its members, and the two halves of a split button
meet. Floating leaves the applications their own corners.

Any style goes with any theme. `ame desktop style neon` keeps neon whatever the theme, and
`ame desktop style theme` goes back to the theme's own. Run `ame desktop style` alone to pick from a list,
or use the **Style** choice on Control's **Appearance** page. The desktop changes at once. An application
that is already open takes the new shape the next time it starts.

## Glass

The top bar, the dock, menus, the quick settings, notifications, dialogs and the windows you are not using
stand on glass: the wallpaper behind them, blurred once and shared, so the glass costs nothing while nothing
changes. There are three kinds:

| Glass | Looks like |
| --- | --- |
| `frost` | Thick, milky glass with an even tint, as the desktop has always had it |
| `prism` | Less tint and more colour, a lit top edge, a soft glint at the top left and the theme's accent gathering along the bottom, as light does in thick glass. The one the desktop starts with |
| `clear` | The thinnest glass and the brightest rim, with a glow up the left side |

Whichever you pick, text on the glass stays readable on any wallpaper: over a bright picture the glass
darkens just enough (on a light theme, lightens), and over the image's own dark wallpapers it barely changes.
Near its edge the glass bends the picture inwards, as a rounded rim does.

The corners of the shell's own surfaces are continuous: the curve starts further out and eases in, so a
corner reads as one shape rather than a square with a circle cut out of it. Windows keep the corners GTK draws,
a pill stays a pill, and a part a style outlines itself (the hairline and neon styles, the islands' pills)
keeps round corners that match its outline.

Run `ame desktop glass frost`, `ame desktop glass prism` or `ame desktop glass clear`, or
`ame desktop glass` alone to pick from a list, or use the **Glass** choice on Control's **Appearance** page.
The glass changes at once.

Each kind of glass also has its own strength: how deep its blur is, from light to deep, and how dense its tint, from
clear to dense, each from half to one and a half times its own. Control's **Glass blur** and **Glass tint** sliders move
the glass you use now, live; each glass keeps its own. Text on the glass stays readable at every strength.

    ame desktop glass-strength blur 130            # the glass in use; 50 to 150 per cent
    ame desktop glass-strength tint 70 frost       # one glass by name
    ame desktop glass-strength                     # all of them
    ame desktop glass-strength reset               # the glass in use, back to its own

## Comfort

For a desktop that is calmer or clearer, Control's **Appearance** page has a **Comfort** group, and
`ame desktop comfort` alone shows how each of these is set:

- **Reduce transparency** draws every glass surface in the theme's solid colours. Nothing is blurred and the
  glass's pictures are let go, so it also costs less: `ame desktop comfort reduce-transparency on`, and `off`
  to go back.
- **Reduce motion** makes Amethystora's own animations that travel, such as the dock's and a window going down to
  the dock, fade in place instead: `ame desktop comfort reduce-motion on`. It is GNOME's own Reduced Motion
  setting, which apps follow too. To stop every animation, turn **Animations** off in Settings, under
  **Accessibility**; that counts as reduce motion as well. So does a computer whose screen is drawn without a
  graphics card's help (some virtual machines) or that is being used through a remote desktop: GNOME turns its
  animations off there by itself, whatever the switch shows.
- **Increase contrast** is GNOME's High Contrast, also in Settings under **Accessibility**: the glass turns solid,
  every surface gets a strong outline in the text colour, dimmed text is shown plain and controls are outlined.
  libadwaita apps follow it too (see [Colours in applications](#colours-in-applications)):
  `ame desktop comfort increase-contrast on`.
- **The power saver**, the profile you pick in the quick settings' power menu, makes the glass solid and every
  animation take half its time for as long as it is on. Nothing here turns the power saver on by itself.
- **Animation speed**: snappy, normal (the image's), relaxed, or off, which changes everything at once without
  touching GNOME's own Animations switch: `ame desktop animation-speed relaxed`. Reduce motion still turns travel
  into fades, whatever the speed.
- **Text size**, from 90 to 150 percent, apart from the screen's scale, which stays whole: `ame desktop text-size
  115`. It is GNOME's own text scaling: the shell and the Amethystora apps follow it at once, other apps at their next
  start.

### The light desktop

The glass and the motion suit a computer with a graphics card. The light desktop keeps the look and drops the costly
parts: the glass is drawn solid, motion is short and simple, and the dock shows no live window previews. A computer
that draws the screen without a graphics driver, or has less than 8 GB of memory, is offered it once, in a
notification shortly after the first login (and again if its graphics change); either answer is remembered.
Control's **Light desktop** switch, or:

```bash
ame desktop light-desktop on    # off to go back
```

## Seeing and reading

GNOME has a screen reader, zoom and large text, in Settings under **Accessibility**. Amethystora adds three things
there are no switches for, in Control's **Appearance** page, group **Seeing and reading**, and with
`ame desktop access`, which alone shows how each is set. All three are off until you choose them.

### Colour filters

A filter over the whole screen, every app and the desktop itself:

- **Red (protan)** and **Green (deutan)** pull apart the reds and greens that an eye with a red or a green weakness
  mixes up, by moving what that eye misses into the colours it sees well: `ame desktop access filter deutan`.
- **Blue-yellow** does the same for blues and yellows: `ame desktop access filter tritan`.
- **Grey** draws everything in grey, for fewer distractions: `ame desktop access filter grey`.

The strength is **Half**, **¾** or **Full** (`ame desktop access strength 75`; any number from 25 to 100). While a
filter is chosen, quick settings has a **Colour filter** tile: its switch turns the filter off and on again, and its
menu picks another filter or strength. `ame desktop access filter none` takes the filter and the tile away.

The pointer is not filtered (in some virtual machines, where the desktop draws it, it is). Zoom's magnified picture is
filtered like the rest. **Screenshots are saved without the filter**, in the apps' own colours, so that a picture you
send looks right to everyone; on the screen, the screenshot tool is filtered like the rest. **Recordings and screen
shares may show the apps' own colours**: whether one of a whole screen shows the filter depends on the program that
makes it, and one of an area or of a window never does. The lock screen is shown without the filter, and so is a
screen in HDR, which the filter does not work with yet (it says so the first time). While a filter is on, a game shown
full screen is drawn through the desktop like any window, and each screen is copied and drawn once more for every
frame, which costs a little speed and battery.

### The reading ruler

A band that follows the pointer, for anyone who loses their line: dyslexia, tired eyes, long pages. It lies across the
window you are using while the pointer is over it, else across the screen, and is as tall as a line of text: taller
when you make the text larger in Settings. **Super+Ctrl+R** hides it and shows it again.

- **Band**: a highlighter's yellow band: `ame desktop access ruler band`.
- **Dim the rest**: the window (or screen) above and below the line is veiled: `ame desktop access ruler dim`.
- **Underline**: a line in the theme's accent under the line you read: `ame desktop access ruler underline`.

`ame desktop access ruler off` turns it off, and its shortcut with it. The ruler shows in screenshots.

### Reading aloud

With `ame desktop access read-aloud on`, select some text in any app and press **Super+Ctrl+S**: it is read aloud with
the voice the screen reader uses, on this computer; nothing is sent anywhere, and no screen reader needs to be on. The
live pill says **Reading aloud** with the speed, which steps through 1×, 1.5×, 2× and 0.75× from the next sentence
(the speeds are about that), and **Stop**; **Super+Ctrl+S** again stops too. Stopping also cuts short the sentence the
screen reader is saying at that moment, if one is. A long selection is read up to about
twenty thousand characters. The voice is eSpeak NG through Speech Dispatcher, the same as Orca's; other voices that
Speech Dispatcher knows can be set in its own settings.

### Click with the keyboard

With `ame desktop access letters on` (or **Click with the keyboard** in [Control](control.md)), press **Super+Alt+K** in
any window: a small letter appears on each of its buttons, links, boxes, tabs and menu items. Type the letters of one
to click it; a text field gets the cursor instead. `Backspace` takes back a letter, and `Escape`, a click or a letter
that matches nothing takes them away. With many controls each has two letters.

The letters come from what the window tells the accessibility tree, the one screen readers use, and only while they
show: nothing is read before **Super+Alt+K** and nothing is kept after. Apps made with GTK, such as Settings, Files and
Amethystora's own, say the most. Firefox and apps built on Chromium or Electron fill in their tree only while a screen
reader runs, so they may show no letters. `ame desktop access letters off` turns it off, and its shortcut with it.

## Fonts

Each theme is written in a font of its own, used by the desktop and the applications:

| Theme | Font |
| --- | --- |
| Amethystora, Amethystora Light | Quicksand |
| Catppuccin Mocha, Catppuccin Latte | Nunito |
| Everforest | Manrope |
| Gruvbox | JetBrains Mono |
| Matte Black | Geist |
| Nord | Inter |
| Rosé Pine Dawn | Figtree |
| Tokyo Night | Space Grotesk |

`ame desktop font "Space Grotesk"` keeps a font whatever the theme. Any font installed on the computer
works, by its name as Fonts shows it. `ame desktop font theme` goes back to the theme's own, and
`ame desktop font` alone offers the fonts the themes use, with Adwaita Sans, GNOME's own. Control has the
same choice under **Appearance**. Documents are set in the same font, a size up.

The terminal, code and Text Editor have a monospace font of their own, which goes with the theme's:

| Theme | Monospace font |
| --- | --- |
| Amethystora, Amethystora Light, Gruvbox | JetBrains Mono |
| Catppuccin Mocha, Catppuccin Latte | Fira Code |
| Everforest | DM Mono |
| Matte Black | Geist Mono |
| Nord | IBM Plex Mono |
| Rosé Pine Dawn | Victor Mono |
| Tokyo Night | Space Mono |

`ame desktop mono-font "Fira Code"` keeps one whatever the theme, `ame desktop mono-font theme` goes back,
and Control has the choice too. kitty takes it at once, and so does the top bar of the terminal style.

For reading, any theme can take **Atkinson Hyperlegible Next**, a typeface drawn so that the letters people confuse
look different (capital I, small l and 1, O and 0, rn and m), and its monospace cut **Atkinson Hyperlegible Mono** for
the terminal and code:

```bash
ame desktop font "Atkinson Hyperlegible Next"
ame desktop mono-font "Atkinson Hyperlegible Mono"
```

## Icons

Eight icon packs are drawn for Amethystora, each icon in colours of its own, so that a theme has one
that reads on its ground and suits its style:

| Pack | Draws each icon as | The themes it comes with |
| --- | --- | --- |
| **Glass** | A gradient with a glass sheen | Amethystora |
| **Glass for light** | The same, its colours deepened to read on a light ground | Amethystora Light |
| **Duotone** | A gradient, its details in a contrasting one | Catppuccin Mocha, Gruvbox |
| **Clay** | A soft matte volume, a highlight above and a shade below | Catppuccin Latte, Rosé Pine Dawn |
| **Muted** | Glass in quieter colours with half the sheen | Everforest, Nord |
| **Line** | One even outline, in colours that read on every theme | Matte Black |
| **Neon** | Outlines lit in colours of their own | Tokyo Night |
| **Neon for light** | Neon, its colours deepened for a light ground | None: pick it on a light theme |

They draw Amethystora's apps, GNOME's, the system's, folders, file types,
devices, dialogs and the pictograms of a job, and the apps of other makers by what each one does:
a terminal as a terminal, a mail app as an envelope, a music player as a note.

An app whose icon is its maker's logo keeps it. Firefox, Thunderbird, Steam, a JetBrains IDE and
others like them show the icon their maker drew, exactly as it ships, with no tile or frame added:
a logo is a trademark, and how it looks is its owner's to decide. That is why the dock can mix the two.

`ame desktop icons clay` (or the name of any pack: `glass`, `glass-light`, `neon`, `neon-light`, `duotone`,
`line`, `clay`, `muted`) picks one whatever the theme, and
`ame desktop icons theme` goes back to what the theme chooses. Control has the same choice under
Appearance.

## Pointers

Amethystora draws its own pointers, in three sets:

| Pointer | Looks like | The themes it comes with |
| --- | --- | --- |
| **Halo** | White, with an amethyst rim | Every dark theme |
| **Ink** | Dark, with a white rim | Amethystora Light, Catppuccin Latte, Rosé Pine Dawn |
| **Facet** | Cut like the gem, in two faces of amethyst | None: pick it on any theme |

`ame desktop pointer facet` (or `halo`, `ink`) keeps one whatever the theme, and `ame desktop pointer theme`
goes back to the theme's. Any other pointer you choose in Tweaks is left alone by every theme; to keep Halo, Ink or
Facet whatever the theme, choose it with `ame desktop pointer` rather than in Tweaks. The size is GNOME's, in
Settings under Accessibility.

![Shake to find](media/shake-to-find.webm)

**Shake to find.** Off until you turn it on with `ame desktop pointer-shake grow`, or in Control. Then, when you have
lost the pointer, shake the mouse quickly from side to side and it swells to three and a half times its size, then
settles a moment after you stop. `ame desktop pointer-shake beacon` sends rings out from it instead, and `ame desktop
pointer-shake off` turns it off again. Nothing runs while the mouse is still, a shake made with a
button held (a drag, a drawing) never counts, and GNOME's own way, a press of Ctrl (Settings, Accessibility, Locate
Pointer), works as before.

**Larger on dense screens.** On a screen with small pixels shown at 100%, such as a 14-inch 2560×1600 laptop panel or
a 27-inch 4K monitor, the pointer is drawn one size larger (32 in place of 24), so that it is as easy to find as the
text is to read. A screen scaled to 200% already doubles it, and a size you chose in Settings is always kept.
`ame desktop pointer-dense off` turns it off. Control has all three under Appearance.

## Rounded screen corners

Most laptop screens are square at the corners. With rounded corners on, the desktop draws a small black curve in
each corner of the laptop's own screen, over everything, so that the whole screen reads as one rounded piece of
glass. A monitor on a cable, and a desktop computer's screen, stay square. It is off until you turn it on:
`ame desktop screen-corners on` (and `off`), or **Rounded screen corners** in Control, under Appearance.

The corners are four tiny shapes, drawn once, and cost nothing while you work. One thing to know: something drawn
over a fullscreen window keeps GNOME from handing that window to the screen directly, so a fullscreen game or video
takes a little more work to show while the corners are on. Screenshots of the whole screen include them, since they
are what the screen shows.

## Change a theme, or make your own

Themes ship read-only in `/usr/share/amethystora/themes`. A folder of the same name in
`~/.config/amethystora/themes` is laid over the top, so you only write the files you want to change.
A folder with a name of its own is a new theme. Each file is small and optional, except the palette:

| File | Holds |
| --- | --- |
| `colors.toml` | The palette: `accent`, `foreground`, `background`, `cursor`, `selection_foreground`, `selection_background`, and `color0` to `color15`. A palette whose `color1`, `color2` and `color3` are not a red, a green and a yellow can add `error`, `success` and `warning`, which the applications' status colours come from |
| `light.mode` | Present, and empty, when the theme is light |
| `pair.theme` | The theme `Super+Ctrl+D` flips to |
| `accent.theme` | GNOME's accent colour: blue, teal, green, yellow, orange, red, pink, purple or slate. Without it the closest one is picked |
| `gtk.theme` | A GTK theme name, instead of GNOME's own look |
| `icons.theme` | An icon theme, instead of Glass (or Glass for light, on a light theme) |
| `style.theme` | The style: `islands`, `floating`, `hairline`, `capsule`, `terminal`, `segments` or `neon`. Without it, floating |
| `font.theme` | The font's name, instead of Adwaita Sans |
| `mono.theme` | The monospace font's name, instead of JetBrains Mono |
| `cursor.theme` | A cursor theme, instead of Halo (Ink for a light theme) |
| `vscode.theme` | A VSCodium colour theme to switch to, instead of the palette's colours merged into its settings |
| `shell-workspaces.css`, `shell-meters.css` | The top bar's colours for the workspaces and the meters, in GNOME Shell's CSS, instead of the ones drawn from the palette |
| `backgrounds/` | Wallpapers |
| `backgrounds.list` | Wallpapers kept elsewhere on the system, one path per line. The first is the one the theme opens on |

A theme with no pictures of its own gets four drawn in its colours: a soft glow, hills in a haze, a
contour map and flowing ribbons.

A theme of your own takes three commands:

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

Edit the colours, run `amethystora-theme set sunset` again, and look at the result.

## Make a theme

Control's Appearance page has **Make a theme**, which does the same in a window. Start from any theme, or from **The
wallpaper** (the current theme's palette with the colours of the picture you are looking at), then pick the ground,
the text, the accent, the selection and the terminal's sixteen colours, each in GNOME's colour picker. As you pick,
three pairs are measured the way every theme is drawn: the text on every surface (4.5:1), a label on the accent
(4.5:1) and the outlines of controls (3:1). One that falls short says what it needs, with **Use** and the nearest colour
that passes. **Preview** shows the colours on the theme card's little desktop; nothing changes until you save.

Name the theme and press **Save**: it is written to `~/.config/amethystora/themes/NAME`, the folder described above,
and applied. A name a theme of the system has is refused. In a terminal, the same palette on standard input saves
it:

```bash
grep ' = "#' ~/.config/amethystora/themes/sunset/colors.toml | ame desktop theme save "Sunset Two"
```

### Bring a theme from elsewhere

A colour file written for another program becomes a theme: a kitty `.conf`, an Alacritty `.toml`, a base16 or
base24 `.yaml`, or an Xresources file. Open it with **Import as a Theme** (Files, Open With), press **Import a theme…**
in Control's **Make a theme**, or:

```bash
ame desktop theme import ~/Downloads/kanagawa.conf
```

It shows the colours it found, how many lines named a colour and how many were left out, and the three checks above,
and asks before it writes anything. Only the lines that name a colour are read: an `include`, an `import` or anything
else in the file is counted and left out, nothing in it runs, and only a file on this computer is read, never an
address. A picture beside the file with the same name (`kanagawa.png` beside `kanagawa.conf`) becomes the theme's
wallpaper, read and written again as a PNG.

### A look in one file

**Share this look** in Control (or `ame desktop theme share`) writes the current theme as one file in your home
folder, `NAME.amethystora-look`: its colours, the wallpaper on screen, and the names of your style, icons, fonts,
pointer and accent. It holds no apps, no keys and nothing that runs, unlike [the setup file](setup.md).

Opening such a file is off until you turn it on, since a file from a stranger is still a file from a stranger:
**Open looks from files** in Control, or `ame desktop look-files on`. Then opening one (or
`ame desktop theme look FILE`) shows what it holds and asks before anything changes. A style, icon pack, font or
pointer this computer does not have is left out and said. The file is read piece by piece and never unpacked: one that
holds anything but its three pieces, or more than 25 MB, is refused whole, and its picture is read and written again as
a PNG.

## Drawn wallpapers

Seven patterns can be drawn in the theme's own colours: **Facets**, **Waves**, **Contour**, **Mesh**, **Grain**,
**Dots** and **Bands**. Pick one in Control (Appearance, **Drawn wallpapers**) or:

```bash
ame desktop pattern facets
ame desktop pattern shuffle
ame desktop pattern off
```

The drawing is made once, when the theme is applied, and drawn again when the theme or the accent changes; nothing
is drawn while you are not changing anything. **Another drawing** (`ame desktop pattern shuffle`) picks another
pattern and draws it afresh. The drawing comes before the theme's own pictures, so `Super+Ctrl+Space` still goes on to
them, and with [Daylight](#daylight) on it is shown at dawn, by day, at dusk and at night in tones of the same palette.
`ame desktop pattern off` gives the theme's own pictures back.

## Light by day, dark by night

A theme with a light and a dark side can follow the day: its light side when the day starts and its dark side when
the night does, revealed by the [transition](#transitions) as any theme switch is. Amethystora and Amethystora Light,
and Catppuccin Mocha and Catppuccin Latte, have both sides; Everforest, Gruvbox, Matte Black, Nord, Rosé Pine Dawn
and Tokyo Night have one, and stay as they are. A theme of your own has a pair when its `pair.theme` names one.

It is off unless you turn it on, in Control (Appearance, **Light by day, dark by night**), or:

```bash
ame desktop day-night on
ame desktop day-night hours 7 19
ame desktop day-night off
```

With `on`, the day starts and ends with Night Light: its own hours, or sunrise and sunset when Night Light follows
them; when Night Light is off, at 07:00 and 19:00. `hours` sets the two hours yourself. Switching to the other side by
hand (`Super+Ctrl+D`) is kept until the next start of the day or night. Open applications take the new colours when
they next start, as with any theme switch.

## Going further

- **How a config is written.** The terminal palette, the prompt, btop's colours and the drawn
  wallpapers are templates in `/usr/share/amethystora/themed`. Copy one to
  `~/.config/amethystora/themed/` and edit the copy to change how every theme renders it.
- **Run something on every switch.** Put an executable in
  `~/.config/amethystora/hooks/theme-set.d/`. It gets the theme's name in `AMETHYSTORA_THEME` and its
  folder in `AMETHYSTORA_THEME_DIR`.
- **Your own prompt stays yours.** If you have written `~/.config/starship.toml` yourself, the theme
  leaves it alone.
- **Never edit `~/.config/amethystora/current/`.** It is rewritten on every switch.

## The boot menu

The boot menu carries the Amethystora artwork too. The boot menu lives outside the image, so the
artwork is put on once, as the machine first starts with it, and shows from the restart after;
once taken off, it stays off:

```bash
ame desktop boot-menu
```

It says whether the artwork is on and asks whether to put it on or take it off; `ame desktop boot-menu on`
and `off` skip the question. The switch on Control's **Appearance** page opens the same in a terminal. Once it is on, image updates keep it up to date.
