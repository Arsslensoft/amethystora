# Styles, fonts and icons

The shape of the desktop, the glass it stands on, its fonts, icons and pointers. Each theme brings its own, and you
can keep any of them whatever the theme.

## Styles

A style is the shape the whole desktop is drawn in: the top bar, menus, quick settings, notifications, dialogs and
prompts, volume and brightness alerts, buttons, switches and entries, the dock and the windows. Each theme comes with one.

![Floating, islands, hairline](media/desktop-styles.webm)

| Style | Looks like | The theme it comes with |
| --- | --- | --- |
| `islands` | A floating pill for each part of the top bar, the workspaces as dots, soft corners everywhere | Amethystora Light, Catppuccin Latte, Nord |
| `floating` | One rounded top bar held off the edge of the screen | Amethystora, Everforest |
| `hairline` | A thin, solid top bar, crisp corners and fine lines | Matte Black |
| `capsule` | The clock in a dark capsule, the rest of the top bar bare over the wallpaper, everything else a pill | Rosé Pine Dawn |
| `terminal` | Monospace, square corners and lined edges, like a terminal | Gruvbox |
| `segments` | The top bar in blocks of the theme's colours | Catppuccin Mocha |
| `neon` | Dark glass outlined and glowing in the theme's accent colour | Tokyo Night |

```bash
ame desktop style          # pick from a list (or Control, Appearance, Style)
ame desktop style neon     # keep neon whatever the theme
ame desktop style theme    # the theme's own again
```

The desktop changes at once; an application already open takes the new shape the next time it starts.

### Styles in applications

Every control in the applications is rounded by the style: buttons, entries, tabs, menu rows, switches, cards and
lists, popovers, dialogs, tooltips, toasts and sliders' troughs. A group of buttons or fields is rounded at its ends
and straight between its members, and the halves of a split button meet. Floating leaves the applications their own
corners.

## Glass

The top bar, the dock, menus, quick settings, notifications, dialogs and the windows you are not using stand on
glass: the wallpaper behind them, blurred once and shared, so it costs nothing while nothing changes.

![Glass](media/glass.webm)

| Glass | Looks like |
| --- | --- |
| `frost` | Thick, milky glass with an even tint, as the desktop has always had it |
| `prism` | Less tint and more colour, a lit top edge, a soft glint at the top left and the theme's accent gathering along the bottom, as light does in thick glass. The one the desktop starts with |
| `clear` | The thinnest glass and the brightest rim, with a glow up the left side |

`ame desktop glass frost` (or `prism`, `clear`), `ame desktop glass` alone to pick from a list, or **Glass** on
Control's **Appearance** page. It changes at once.

Text on the glass stays readable on any wallpaper and at every strength: over a bright picture the glass darkens just
enough (on a light theme, lightens). Near its edge the glass bends the picture inwards, as a rounded rim does.

### Corners

The shell's own surfaces have continuous corners: the curve starts further out and eases in, so a corner reads as one
shape. Windows keep the corners GTK draws, a pill stays a pill, and a part a style outlines itself (hairline, neon,
the islands' pills) keeps round corners that match its outline.

### Glass strength

Each kind of glass has its own blur (light to deep) and tint (clear to dense), each from half to one and a half times
its own. Control's **Glass blur** and **Glass tint** sliders move the glass in use, live; each glass keeps its own.

    ame desktop glass-strength blur 130            # the glass in use; 50 to 150 per cent
    ame desktop glass-strength tint 70 frost       # one glass by name
    ame desktop glass-strength                     # all of them
    ame desktop glass-strength reset               # the glass in use, back to its own

## Fonts

Each theme is written in a font of its own, used by the desktop and the applications; documents are set in it a size up.

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

```bash
ame desktop font "Space Grotesk"   # any installed font, by its name in Fonts, whatever the theme
ame desktop font theme             # the theme's own again
ame desktop font                   # pick from the themes' fonts and Adwaita Sans, GNOME's own
```

Control has the same choice under **Appearance**.

### Monospace

The terminal, code and Text Editor have a monospace font that goes with the theme's:

| Theme | Monospace font |
| --- | --- |
| Amethystora, Amethystora Light, Gruvbox | JetBrains Mono |
| Catppuccin Mocha, Catppuccin Latte | Fira Code |
| Everforest | DM Mono |
| Matte Black | Geist Mono |
| Nord | IBM Plex Mono |
| Rosé Pine Dawn | Victor Mono |
| Tokyo Night | Space Mono |

`ame desktop mono-font "Fira Code"` keeps one whatever the theme, `ame desktop mono-font theme` goes back, and
Control has the choice too. kitty and the terminal style's top bar take it at once.

### Fonts for reading

**Atkinson Hyperlegible Next** is drawn so that the letters people confuse look different (capital I, small l and 1,
O and 0, rn and m); **Atkinson Hyperlegible Mono** is its monospace cut. Any theme can take them:

```bash
ame desktop font "Atkinson Hyperlegible Next"
ame desktop mono-font "Atkinson Hyperlegible Mono"
```

## Icons

Eight icon packs are drawn for Amethystora, each icon in colours of its own, so every theme has one that reads on its
ground and suits its style.

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

```bash
ame desktop icons clay     # glass, glass-light, neon, neon-light, duotone, line, clay or muted
ame desktop icons theme    # what the theme chooses
```

Control has the same choice under Appearance. The packs draw Amethystora's apps, GNOME's, the system's, folders, file
types, devices, dialogs and pictograms, and other makers' apps by what each does: a terminal as a terminal, a mail app
as an envelope, a music player as a note.

### Makers' logos

An app whose icon is its maker's logo keeps it. Firefox, Thunderbird, Steam, a JetBrains IDE and the like show the icon
their maker drew, exactly as it ships, with no tile or frame: a logo is a trademark, and how it looks is its owner's to
decide. That is why the dock can mix the two.

## Pointers

Amethystora draws its own pointers, in three sets:

| Pointer | Looks like | The themes it comes with |
| --- | --- | --- |
| **Halo** | White, with an amethyst rim | Every dark theme |
| **Ink** | Dark, with a white rim | Amethystora Light, Catppuccin Latte, Rosé Pine Dawn |
| **Facet** | Cut like the gem, in two faces of amethyst | None: pick it on any theme |

`ame desktop pointer facet` (or `halo`, `ink`) keeps one whatever the theme, and `ame desktop pointer theme` goes
back to the theme's. A pointer you choose in Tweaks is left alone by every theme; to keep Halo, Ink or Facet whatever
the theme, choose it with `ame desktop pointer` rather than in Tweaks. The size is GNOME's, in Settings under
Accessibility. Control has all three pointer choices under Appearance.

### Shake to find

Lost the pointer? Shake the mouse quickly from side to side and it swells to three and a half times its size, then
settles a moment after you stop. Off until you turn it on, here or in Control.

![Shake to find](media/shake-to-find.webm)

```bash
ame desktop pointer-shake grow     # beacon sends rings out instead; off turns it off
```

Nothing runs while the mouse is still, and a shake with a button held (a drag, a drawing) never counts. GNOME's own
way, a press of Ctrl (Settings, Accessibility, Locate Pointer), works as before.

### Larger on dense screens

On a screen with small pixels shown at 100% (a 14-inch 2560×1600 laptop, a 27-inch 4K monitor) the pointer is drawn
one size larger, 32 in place of 24. A screen scaled to 200% already doubles it, and a size you chose in Settings is
always kept. `ame desktop pointer-dense off` turns it off.

## Rounded screen corners

A small black curve in each corner of the laptop's own screen, over everything, so the screen reads as one rounded
piece of glass. A monitor on a cable, or a desktop computer's screen, stays square. Off until you turn it on:
`ame desktop screen-corners on` (and `off`), or **Rounded screen corners** in Control, under Appearance.

The corners are four tiny shapes drawn once, and cost nothing while you work. But something drawn over a fullscreen
window keeps GNOME from handing it to the screen directly, so a fullscreen game or video takes a little more work while
they are on. Whole-screen screenshots include them.
