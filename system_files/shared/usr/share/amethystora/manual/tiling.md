# Tiling windows

The desktop arranges the windows for you. Each workspace is a strip of windows side by side at full height, and
the strip can be wider than the screen.

### The strip

A new window opens to the right of the one you are on, windows never overlap, and the strip scrolls to keep the
focused window in view, marked by a frame:

```
  ┌────────┬──────────────┬──────────────────────┬────────┐
  │  Mail  │   Terminal   │       Browser        │  Chat  │
  └────────┴──────────────┴──────────────────────┴────────┘
           ╰──────────── the screen ─────────────╯
```

Mail and Chat are still open, just off the edge: `Super+Left` scrolls back to Mail.

![Scroll along the strip](media/tiling-strip.webm)

### With the pointer and the touchpad

- Push the pointer against the left or right edge to see what is beyond it, and click a window there to go to it.
- Swipe sideways with three fingers to slide the strip, up for the overview, and down for the workspaces you have
  been on, as a pile of cards.
- Drag a window by its title bar and drop it where you want it in the strip, or against a screen edge to see where
  it will go first. The mouse wheel moves along the strip while you hold it.
- A window you resize by its edge onto another screen floats.

### The keys

The keys follow one pattern: `Super` and an arrow moves to another window, add `Ctrl` and the window moves instead,
add `Shift` and you go to another monitor.

## Moving around

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

## Several monitors

With two monitors or more, each shows a workspace of its own, and a workspace is on one monitor at a time. The
monitor the pointer is on is the one you work on: move the pointer there and its workspace is the current one, with
no animation.

### Choosing workspaces across monitors

- `Super+Page Up` and `Super+Page Down` go through the workspaces of the monitor you are on and the empty ones, and
  the one you choose stays on that monitor.
- A workspace chosen another way (`Super` and a number, the top bar, an app's window) comes up on the monitor it was
  last on, and the pointer goes there. When no monitor showed it, the monitors slide to it as GNOME's do.
- Swapping or moving a workspace between monitors needs one workspace more than you have monitors.
- Dynamic workspaces (Settings, Multitasking) stay dynamic: a workspace a monitor shows is kept even when empty, and
  there is always one more than you have monitors.
- Unplug the monitor you are working on and the most recent workspace of the others becomes the current one. A
  monitor plugged back in shows the workspace it had, if it is still there.

### A top bar and a dock on every monitor

The top bar and the dock start on the main monitor only. `ame desktop top-bar screens on` gives every other monitor a
top bar, and `ame desktop dock screens every` a dock; Control has both. `ame desktop top-bar screens off` takes the
other bars away.

Such a bar is as tall and as round as the main one, with the workspaces (the one that monitor shows lit) and the
clock. Click a workspace there to show it on that monitor; one another monitor shows swaps places with it. Drop a
window from the overview on a workspace to send it there. The tray, quick settings and the menus stay on the main
monitor's bar, and `Super+Ctrl+B` hides the bar on the monitor showing that workspace. In the classic layout, where
the other monitors keep their windows on every workspace, their bars show the clock alone.

## Arranging

![Move a window along the strip](media/tiling-move-window.webm)

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

## Sizing

![Step the width](media/tiling-width-steps.webm)

| Key | Does |
| --- | --- |
| `Super+R` | Step the width through 38%, 50% and 62% of the screen less its margins (`Super+Alt+R` steps back) |
| `Super+Shift+R` | The same for the height of a window in a column (`Super+Alt+Shift+R` steps back) |
| `Super` and `+` / `-` | Widen / narrow the window a step; with `Shift` as well, its height |
| `Super+F` | Fill the width of the screen, and back; maximising a window does the same |
| `Super+C` | Centre the window on the screen |
| `Super+Shift+C` | Where the strip scrolls a focused window to: just into view, the centre or the edge |
| `Super+Shift+W` | Where new windows open: to the right, to the left, or below this one |

## Floating windows

Dialogs float above the strip on their own. `Super+Ctrl+Escape` lifts the focused window out of the strip so that
it floats too, and puts it back; every window's menu, a dialog's too, has the same as **Float above the strip**.
`Super+Alt+Escape` hides every window floated that way at once, and brings them back. A window you put on every
workspace (**Always on Visible Workspace** in its menu) leaves the strip and stays where it is.

## And the rest

| Key | Does |
| --- | --- |
| `Super+N` | A new window of the app you are in |
| `Super+Ctrl+B` | Hide or show the top bar on this workspace |

## Tiling settings

**Tiling settings** on [Control](control.md)'s **Desktop** page, or `ame desktop tiling-settings`, opens the
tiling's settings window. Every change applies at once.

- **Windows**: the gaps between windows and around the screen, the frame around the window you are on, where new
  windows open and which places `Super+Shift+W` goes through, how a workspace scrolls to the window you go to, the
  widths `Super+R` and heights `Super+Shift+R` step through (shares like `38%, 50%, 62%`, or widths like `800px`),
  the width `Super+F` gives, and whether maximising fills the width or the whole screen.
- **Motion**: how long a window takes to glide, the speed of `Super+[` and `Super+]`, the map of the strip and the
  darkening of the other windows while you move around, what the screen edges do (show the window beyond, go to it
  on a click, or after the pointer stays there), the touchpad's swipes (fingers, speed, friction), the two icons in
  the top bar, and the size of the windows in `Alt+Tab`. Off as the desktop comes: the workspace's name in the top
  bar (a click opens the overview, the other button renames it), a line under the top bar marking where the window
  is in the strip, and an overview with only the floating windows or without them.
- **Workspaces**: a picture for every workspace in place of the theme's wallpaper (or none, only each workspace's
  colour), and for each workspace a name, a picture, a colour and a folder. The colour frames the workspace's card
  while you go through the workspaces, and apps started while it is shown start in its folder.
- **Keyboard**: every shortcut, including those left unbound (the next window, the n-th column, the window above or
  else the previous workspace, centring both ways, closing, and a switcher through the windows used last, drawing
  each where its strip would show it). Click one and press the new keys. The first time, GNOME asks whether the
  window may have the desktop's keys while you press them: allow it. A shortcut another one also uses is marked.
- **Window rules**: a window picked by its class or title as it opens can float, open at a width, or open on a
  workspace, once or every time. `/a pattern/` matches part of a name, and `*` any window no other rule picks.

`gsettings` changes them too: the settings are in `org.gnome.shell.extensions.amethystora-ui.tiling`, and the
shortcuts in `org.gnome.shell.extensions.amethystora-ui.tiling.keybindings`, which [the setup file](setup.md) keeps.

## Classic windows

For windows that float and overlap, as on most desktops, switch to the classic layout:

```bash
ame desktop layout classic   # ame desktop layout tiling brings the strip back
```

It turns the tiling off, and updates leave it off. Windows open where GNOME puts them and you drag them by the title
bar. `Super+Up` maximises and `Super+Down` restores, `Super+Left` / `Super+Right` put a window on that half of the
screen, and `Super+Shift+Left` / `Super+Shift+Right` move it to the next monitor. A notification at your first login
offers the same choice, and [Control](control.md)'s **Desktop** page has it any time, as **Window layout**. See
[Windows in the classic layout](classic-layout.md).
