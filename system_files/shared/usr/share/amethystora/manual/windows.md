# Windows

The keys and menu entries that work on any window, in both layouts unless a section says otherwise. For the strip,
see [Tiling](tiling.md); for floating windows, [the classic layout](classic-layout.md).

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

The three lights on the right of every title bar minimise, maximise and close the window: amber, green and red in
every theme, grey while the window is not the one you are using. Each shows its sign under the pointer.

## Keep a copy in view

**Keep a copy in view**, in a window's menu (right-click its title bar, or `Alt+Space`), puts the window out of sight
and keeps a small live copy of it on top of every workspace, in a corner. Drag it to another corner; click it to
bring the real window back. Some apps stop drawing while out of sight, and the copy then shows their last picture.

## Dimming

`ame desktop windows dim on` draws the windows behind the one you are using a little darker, so the eye goes to the
right one. A video an app draws on its own may stay bright.

## Drop-down terminal

`Super+Alt+Return` slides a terminal down from the top of the screen, over whatever is open and on any workspace; the
same key slides it away, still running. Pressed while another window has the keys, it gives the terminal the keys
instead.

![The drop-down terminal](media/drop-down-terminal.webm)

It is kitty, started at the first press, with no title bar; type `exit` to close it, and the next press starts
another. `ame desktop windows drop-down off` turns it off, and a terminal that was down stays open as an ordinary
window.

### The scratchpad

In the classic layout any window can join the terminal: **Move to the scratchpad** in the window's menu puts it out
of sight, and `Super+Alt+Return` then shows and hides it with the terminal, each where it was left, on every
workspace and above the rest. **Take out of the scratchpad** gives it back as it was. The tiling has its floating
windows for this instead (`Super+Alt+Esc`). `ame desktop windows scratchpad off` turns the scratchpad off.

## Keep on top

`Super+Alt+P` keeps the window you are using above every other window, and again lets it go. Such a window has a thin
ring in the theme's accent and a pin on its top corner, which you click to let it go. **Always on Top** in the
window's menu does the same. `ame desktop windows keep-on-top off` turns the key and the ring off; windows already on
top stay so.

## See-through windows

With `ame desktop windows see-through on`, hold `Super+Alt` and scroll over a window: down to see more of what is
behind it, as far as 40 %, up to make it solid again. The window forgets it when it closes.

![See through a window](media/see-through-window.webm)

- To have an app's windows always open see-through, give it a level: `ame desktop windows see-through-app
  kitty.desktop 92`, or the terminal's own slider in [Control](control.md); `off` in place of the number takes the
  rule away.
- While the screen is shared, a window the [share guard](screen-privacy.md#share-guard) covers is solid.
- `Super` and the scroll wheel alone still change workspace.

## Window frame

With `ame desktop windows frame on`, the window you are using has a still line round it, from the theme's warm colour
through its accent to its cool one, in the classic layout (the tiling frames it its own way). A maximised window, and
one kept on top, have none.

## Sizes for recording

**Size for recording**, in a window's menu, makes the window exactly 1280 × 720, 1920 × 1080, 1080 × 1080 or
1080 × 1920 and centres it on its screen, for screencasts and screenshots that line up. The sizes are the screen's own
pixels, whatever its scale; one that does not fit is greyed out. In the tiling it is offered for floating windows
only. `ame desktop windows recording-sizes off` takes it out of the menu.

## Reopen

Closed the wrong window? `Super+Alt+T` opens that app again, on its workspace and, in the classic layout, at the place
and size its window had. Press it again for the one before, up to five back.

![Reopen a closed window](media/reopen-closed.webm)

Only the app is started: what it had open is the app's own to bring back. What closed is kept in memory only and
forgotten when you log out. `ame desktop windows reopen off` turns it off and forgets.

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

`Super+Ctrl+Shift+Esc` lists the running apps, with how much processor and memory each uses now, counting everything
it started (a terminal's commands, a browser's helpers). An app that does not respond comes first, marked **Not
responding**. Pick one and press **Force quit**: the card asks once more, naming it, since anything not saved in it is
lost. It ends only that app's own processes. `ame desktop force-quit off` turns the key off.

### When an app stops responding

The desktop asks over its window whether to **Wait** or **Force quit**. Waiting is often enough, and is what `Esc` and
`Enter` do. While the agentic features are on, **Ask the agent why** opens the agent on that app, to find out what it
is waiting for without ending it (see [the AI agent](ai-agent.md)). `ame desktop frozen-dialog off` brings back
GNOME's own question.

## Motion

Minimise a window and it gathers into a drop of glass that arcs down into its icon in the dock, where a ring ripples
out. Bring it back, from the dock, `Alt+Tab` or anywhere else, and the drop rises and swells back into the window. An
app opened from the dock grows out of its icon the same way; click the icon again half-way and the window turns round
from where it is.

![Minimise into the dock](media/minimise-droplet.webm)

Changing workspace has depth: the windows travel a whole screen, the wallpaper only a third of the way, and the
windows step back a little as they go.

```bash
ame desktop minimize shrink          # GNOME's own: the window shrinks into its icon
ame desktop minimize droplet         # back to the drop
ame desktop workspace-switch slide   # each workspace slides in whole
```

With [Reduce motion](comfort.md#comfort) on (`ame desktop comfort reduce-motion on`), windows fade in place and
workspaces crossfade. With animations turned off in Settings, under Accessibility, nothing moves at all.

### Opening and closing

A window can arrive and leave its own way: **Pop** grows it into place from a little smaller, **Slide** raises it a
little into place, **Fade** only fades it in; closing plays it backwards. Dialogs keep GNOME's. It is GNOME's own
until you choose one (Control's **Opening and closing**):

```bash
ame desktop window-motion pop   # or slide, fade, gnome
```

How fast all of this plays is the [animation speed](comfort.md#comfort).
