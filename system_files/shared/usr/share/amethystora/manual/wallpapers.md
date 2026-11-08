# Wallpapers

Every theme comes with three wallpapers of made-up places drawn in its own colours, and switching to a theme puts its
first one on the desktop. `Super+Ctrl+Space` cycles through them.

### The theme's pictures

A neon city in the rain for Tokyo Night, an aurora over a mountain lake for Nord, misty pines for Everforest, a
desert sunset for Gruvbox, peaks under a pastel moon for Catppuccin, an eclipse for Matte Black, and so on. The two
Amethystora themes open on a cut stone's facets, and also have the crystal field and a night sky of violet smoke;
GNOME's Settings offers those under Appearance.

### Your own pictures

Put pictures (JPG, PNG, WebP or SVG) in a folder named after the theme. They come after the theme's own:

```bash
mkdir -p ~/.config/amethystora/backgrounds/amethystora
cp ~/Pictures/mountains.jpg ~/.config/amethystora/backgrounds/amethystora/
amethystora-theme bg list                 # what the current theme cycles through
ame desktop background <picture>          # set one of those; with no picture, the next one
```

## Daylight

A theme's first wallpaper follows the time of day: the same place at **dawn**, by **day**, at **dusk** and at
**night**, each drawn in the theme's own colours. On unless you turn it off.

![Dawn to night](media/daylight.webm)

Dawn is fully shown at 06:30, day at 12:00, dusk at 18:45 and night at 22:00, and GNOME fades from one to the next over
the three quarters of an hour to an hour and a half before each. The lock screen shows the same picture. Only the
wallpaper changes: the theme stays light or dark. The Amethystora theme's day is still a dark amethyst; Amethystora
Light stays lilac through a peach dawn, a rose dusk and a pale moonlit night.

### Daylight and Night Light

When Night Light has hours of your own (Settings, Displays, Night Light, Manual Schedule), dawn comes when Night Light
ends and dusk when it starts. With Night Light's sunset schedule, which would need your location, the times above are
kept. On the day the clocks change, the pictures come an hour early or late until the next day.

### Turning it off

`ame desktop daylight off`, or Control, Appearance, **Daylight wallpapers**: the first wallpaper then stays as drawn.
Daylight only ever shows the theme's own first picture: one of your own, set in Settings or with
`ame desktop background`, is never replaced. `Super+Ctrl+Space` goes on to the theme's second picture, and round again
to Daylight.

## Living wallpapers

A slow movement over your wallpaper, in the theme's colours: **Aurora** (bands of light swaying across the sky),
**Drifting facets**, **Rain on glass** or **Slow dusk**. Off until you pick one, since each frame costs a little power.

```bash
ame desktop living-wallpaper aurora     # or facets, rain, dusk; off for the wallpaper as it is
```

Or Control, Appearance, **Living wallpaper**. It moves only while you can see it: it is not drawn while a window fills
the screen, and holds still in the overview, on battery, in the power saver, while a game is in front (Games) and with
reduced motion. A workspace with a wallpaper of its own shows that instead, and the glass is still cut from the
wallpaper under it.

## Transitions

A new theme or wallpaper is revealed, not swapped: the screen holds still while everything changes underneath, then
the new desktop opens out in a circle from the pointer, with a glow in the accent colour along the edge. It works
whichever way you make the change: the keys, GNOME's Settings, or the Dark Style button in the quick settings.

![The next theme](media/theme-cycle.webm)

```bash
ame desktop transition         # pick one from a list
ame desktop transition wave    # or name it
ame desktop transition grow 800  # and its time in milliseconds, 200 to 5000 (default 700)
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

Control's **Appearance** page has a slider for the time. There are no transitions while animations are off in
Settings, Accessibility. They come from the Amethystora Transitions extension, which Extension Manager can turn off.

## Drawn wallpapers

Seven patterns drawn in the theme's own colours: **Facets**, **Waves**, **Contour**, **Mesh**, **Grain**, **Dots**
and **Bands**. Pick one in Control (Appearance, **Drawn wallpapers**) or:

```bash
ame desktop pattern facets
ame desktop pattern shuffle   # Another drawing: another pattern, drawn afresh
ame desktop pattern off       # the theme's own pictures back
```

The drawing is made when the theme is applied and again when the theme or accent changes; nothing is drawn
otherwise. It comes before the theme's own pictures, so `Super+Ctrl+Space` still reaches them, and with
[Daylight](#daylight) on it shows dawn, day, dusk and night in tones of the same palette.

## Light by day, dark by night

A theme with a light and a dark side can follow the day: light when the day starts, dark when the night does,
revealed by the [transition](#transitions). Off unless you turn it on.

![Light by day, dark by night](media/day-night.webm)

```bash
ame desktop day-night on          # or Control, Appearance, Light by day, dark by night
ame desktop day-night hours 7 19  # your own hours
ame desktop day-night off
```

With `on`, day and night start with Night Light: its own hours, or sunrise and sunset when it follows them; with Night
Light off, at 07:00 and 19:00. Switching by hand (`Super+Ctrl+D`) is kept until the next start of day or night. Open
applications take the new colours when they next start.

Amethystora and Amethystora Light, and Catppuccin Mocha and Latte, have both sides. Everforest, Gruvbox, Matte Black,
Nord, Rosé Pine Dawn and Tokyo Night have one, and stay as they are. A theme of your own has a pair when its
`pair.theme` names one.
