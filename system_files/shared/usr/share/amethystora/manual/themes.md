# Themes

A theme is one palette that paints the whole desktop at once: GNOME's light or dark style and accent, the terminal,
the prompt, the top bar, the dock, the wallpaper, VSCodium and this manual. Each theme also brings its own
[style](styles.md#styles), [font](styles.md#fonts), [icons](styles.md#icons) and [pointer](styles.md#pointers), and
you can keep any of them whatever the theme (a pointer you choose yourself in Settings stays too).

## Switching

Press `Super+Ctrl+T` for the next theme: the new one opens out from the pointer over the old.

[Show me](guide:change-the-look)

![The next theme](media/theme-cycle.webm)

| Key | Does |
| --- | --- |
| `Super+Ctrl+Shift+Space` | Pick a theme |
| `Super+Ctrl+T` | The next theme, round again to the first at the end |
| `Super+Ctrl+D` | Flip between the light and dark version of the theme |
| `Super+Ctrl+Space` | Next wallpaper of the current theme |

```bash
ame desktop theme                   # pick one from a list, or name it: ame desktop theme Nord
ame desktop theme next              # step to the next theme, round again at the end
amethystora-theme list              # the themes, with the current one marked
amethystora-theme set "Tokyo Night" # apply one by name
amethystora-theme toggle            # light or dark
amethystora-theme current           # which one is on
amethystora-theme reload            # apply the current one again, after editing its files
```

[Control](control.md) (`Super+Ctrl+Shift+C`), **Appearance**, shows every theme in its own colours and the current
theme's wallpapers. Amethystora's own apps change colour with the theme at once.

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

`Super+Ctrl+D` flips a theme to its partner (Catppuccin Mocha and Latte, Amethystora and Amethystora Light); a theme
without one flips to Amethystora in the other mode. A light theme gives the dock a faint dark tint, so its icons stand
out on a pale wallpaper. Every window has the three coloured lights in its corner.

The two Amethystora themes bring the Amethystora GTK theme for GTK 3 applications that draw with a theme of their
own; the others use adw-gtk3, GNOME's own look, in the theme's colours.

## Colours in applications

The theme's colours reach every application, not only Amethystora's own, at a contrast that reads:

- **GNOME's and libadwaita's applications** (Files, Settings, Text Editor, Calendar and most Flatpaks) take the
  theme's surfaces, text, accent and status colours in place of Adwaita's grey. The accent is the theme's own, not
  the nearest of GNOME's nine. Text on it, links, dimmed labels, errors and field outlines are moved in lightness
  until they read (4.5 to 1 for text, 3 to 1 for a line) on every surface; the hue stays the theme's.
- **GTK 3 applications** that draw with adw-gtk3 take the same palette.
- **KDE and Qt applications** take it from `~/.config/kdeglobals` (a KDE Flatpak too). A `kdeglobals` with colours
  of your own, which this did not write, is left alone.
- **VSCodium**, on the developer image, takes the editor, sidebar, status bar, terminal and syntax colours, merged
  into your settings over Default Dark Modern or Default Light Modern. A colour of yours the theme does not name stays.
- **The terminal, the prompt, btop and the pointer** follow too: Halo on a dark theme, Ink on a light one (see
  [Pointers](styles.md#pointers)).

### When applications change

GNOME Shell, Amethystora's apps and the terminal change at once. An application that is already open takes a new
theme's colours, and a new style's corners, the next time it starts.

While High Contrast is on (Settings, Accessibility), libadwaita applications keep their own high-contrast colours
and the palette steps aside until it is off.

## Accents

Any theme can take one of twelve accents in place of its own: Amethyst, Iris, Sky, Lagoon, Jade, Moss, Amber, Ember,
Coral, Rose, Orchid and Slate. The accent colours primary buttons, switches that are on, checked boxes, selected
rows, the current workspace in the top bar, sliders and the focus ring, in the shell, the applications, the prompt
and Amethystora's apps. Each is checked against every surface of every theme, so text on it reads at the contrast
WCAG asks.

```bash
ame desktop accent jade     # or choose in Control, Appearance, Accent
ame desktop accent theme    # Follow the theme: the theme's own again
```

The accent is kept from theme to theme. The shell and Amethystora's apps change at once, other applications at their
next start. A chosen accent wins over [colours from your wallpaper](#colours-from-your-wallpaper).

## Colours from your wallpaper

Off unless you choose it: the desktop's ground, the tint of its text, the selection and the accent are worked out
once from the wallpaper.

![Colours from the wallpaper](media/wallpaper-colours.webm)

- **Vivid**: the strongest hue in the picture becomes the accent, and the ground takes the hue that covers most of it.
- **Soft**: the same hues, quieter.
- **Mono**: greys tinted with the picture's main hue; the accent stays the theme's.

```bash
ame desktop colours soft    # vivid, mono, or off for the theme's own
```

Or Control, Appearance, **Colours from the wallpaper**. It works with every theme: light stays light, dark stays dark,
and the theme's fonts, style, icons, pointer and terminal colours stay. The colours are worked out again when the
wallpaper or theme changes, from the picture of the theme's own time when [Daylight](wallpapers.md#daylight) shows it,
so they do not change through the day. A plain-colour wallpaper leaves the theme's own colours.

## An accent per workspace

Off unless you turn it on: each workspace has an accent of its own, and the shell and Amethystora's apps take it once
the workspace has slid into place (once per change, not while it slides). Other applications read the accent when
they start, so they keep the theme's. The first workspace follows the theme; the others are Sky, Coral, Jade, Amber
and Orchid until you choose.

```bash
ame desktop accent-per-workspace on        # or Control, Appearance, An accent per workspace
ame desktop accent-per-workspace 3 coral   # the accent of workspace 3
```

To choose the accent of the workspace you are on, right-click its label in the top bar and open **Accent**.
