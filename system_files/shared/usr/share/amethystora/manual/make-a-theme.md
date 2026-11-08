# Change a theme, or make your own

Change a theme by writing only the files you want different, make one in Control's window, or bring one from another
program.

### A theme's files

Themes ship read-only in `/usr/share/amethystora/themes`. A folder of the same name in `~/.config/amethystora/themes`
is laid over the top, so you only write the files you change; a folder with a name of its own is a new theme. Each
file is small and optional, except the palette:

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

A theme with no pictures of its own gets four drawn in its colours: a soft glow, hills in a haze, a contour map and
flowing ribbons.

### A theme in three commands

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

Edit the colours, run `amethystora-theme set sunset` again, and look at the result.

## Make a theme

Control's Appearance page has **Make a theme**, the same in a window. Start from any theme, or from **The wallpaper**
(the current palette with the colours of the picture on screen), then pick the ground, text, accent, selection and the
terminal's sixteen colours in GNOME's colour picker.

As you pick, three pairs are measured as every theme is: text on every surface (4.5:1), a label on the accent (4.5:1)
and control outlines (3:1). One that falls short says what it needs, with **Use** and the nearest colour that passes.
**Preview** shows the colours on the card's little desktop; nothing changes until you save.

**Save** writes the theme to `~/.config/amethystora/themes/NAME` and applies it; a system theme's name is refused. In a
terminal, a palette on standard input saves it:

```bash
grep ' = "#' ~/.config/amethystora/themes/sunset/colors.toml | ame desktop theme save "Sunset Two"
```

### Bring a theme from elsewhere

A colour file for another program becomes a theme: a kitty `.conf`, an Alacritty `.toml`, a base16 or base24
`.yaml`, or an Xresources file. Open it with **Import as a Theme** (Files, Open With), **Import a theme…** in Control's
**Make a theme**, or:

```bash
ame desktop theme import ~/Downloads/kanagawa.conf
```

It shows the colours found, how many lines were read and left out, and the three checks, and asks before writing
anything. Only lines that name a colour are read: an `include`, an `import` or anything else is counted and left out,
nothing in it runs, and only a file on this computer is read, never an address. A picture of the same name beside it
(`kanagawa.png` beside `kanagawa.conf`) becomes the wallpaper, read and written again as a PNG.

### A look in one file

**Share this look** in Control (or `ame desktop theme share`) writes the current theme as one file in your home
folder, `NAME.amethystora-look`: its colours, the wallpaper on screen, and the names of your style, icons, fonts,
pointer and accent. It holds no apps, no keys and nothing that runs, unlike [the setup file](setup.md).

Opening such a file is off until you turn it on (a file from a stranger is still a file from a stranger): **Open looks
from files** in Control, or `ame desktop look-files on`. Then opening one (or `ame desktop theme look FILE`) shows what
it holds and asks before anything changes. A style, icon pack, font or pointer this computer lacks is left out and
said. The file is read piece by piece, never unpacked: one holding anything but its three pieces, or over 25 MB, is
refused whole, and its picture is written again as a PNG.

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

The boot menu carries the Amethystora artwork too. It lives outside the image, so the artwork is put on once, when the
machine first starts with it, and shows from the restart after; once taken off, it stays off. Once on, image updates
keep it up to date.

```bash
ame desktop boot-menu         # says whether it is on, and asks
ame desktop boot-menu on      # or off, without the question
```

The switch on Control's **Appearance** page opens the same in a terminal.
