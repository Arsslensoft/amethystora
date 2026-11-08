# The top bar

The bar along the top of the screen holds the logo menu and the workspaces on the left, the clock in the middle, and
the meters, tray and system menu on the right. Every item can be moved or hidden.

## On the left

The Amethystora logo opens a menu of the system's tools: about this computer, this manual, Settings, Bazaar,
Extensions, Mission Center, the terminal and DistroShelf. A middle click on it opens the overview.

Then come the six workspaces, the one you are on highlighted:

- Click another to go there, or the one you are on to show or hide the overview. An empty one opens the overview,
  ready to start something.
- Scroll the mouse wheel, or two fingers on a touchpad, anywhere on the bar for the next or previous workspace.
- Drag a workspace along the row to move it, its windows with it, or drop a window from the overview on one to send
  it there.
- Right-click a workspace to name it (until you do, it is named after its main app).

## In the middle

The clock, with [the weather](today.md#the-weather) beside it once you turn it on. Click it, or press `Super+V`, for
notifications, the calendar and [Today](today.md#today). While something is happening, the clock becomes the
[live pill](pill.md).

## On the right

CPU, memory and network meters, the tray icons, then the system menu: Wi-Fi, sound, power and the rest of the
[quick settings](quick-settings.md).

- **Meters.** Click one for each processor, the memory and the network over the last few minutes, the busiest
  processes, and a button that opens Mission Center. The network meter counts Wi-Fi and wired connections only, so a
  VPN or a container's traffic is not counted twice.
- **Tray icons.** Right-click for the app's menu, double-click to open the app, middle-click for its second action. A
  Flatpak app shows its icon only when it may talk to the tray (`org.kde.StatusNotifierWatcher`), and an old X11 one
  only while X11 apps are allowed.

The meters and workspaces follow the [theme](themes.md)'s colours. The bar can also show [widgets](widgets.md) of your
own, which the AI agent makes for you once you turn it on.

## Over a fullscreen window

A fullscreen video or game keeps the whole screen. Push the pointer against the top edge and the bar comes down over
it, and goes when you move away; `ame desktop top-bar peek off` turns that off. With two monitors or more, the bar and
the dock stay on the main one until you ask for them on the others: see [Several monitors](tiling.md#several-monitors).

## Arranging the bar

Control's **Desktop** page draws the bar as four places, **Left**, **Centre**, **Right** and **Hidden**. Drag an item to
another place, or along its own; each item's menu does the same from the keyboard. The clock and the system menu can
move but never hide, and the privacy dots always stay beside the system menu. **As the image has it** puts everything
back.

![Arranging the bar](media/top-bar-arrange.webm)

```bash
ame desktop top-bar layout show            # each item, and where it is
ame desktop top-bar item clock left 0      # the clock first on the left
ame desktop top-bar item network hidden    # the network meter off the bar
ame desktop top-bar layout reset           # as the image has it
ame desktop top-bar meter processor off    # or memory, network; on brings it back
```

Each meter also has its own switch in Control, under **Top bar**. A meter that is off or hidden reads nothing, and its
menu goes with it, **Network by app** included. The [busy app](power.md) chip has its own
switch and keeps reading the processor until you turn that off too.

The items are `logo`, `workspaces`, `tiling`, `clock`, `processor`, `memory`, `network`, `tray`, `clipboard`, `phone`,
`shield`, `busy`, `keyboard`, `a11y`, `system`, and `widget-NAME` for each of your [widgets](widgets.md). Something
another extension puts on the bar keeps its own place.

## The bar's size

Compact (28 pixels), regular (32, the image's) or roomy (40): everything on the bar grows or shrinks with it. A small
laptop screen gets room back; a large screen across the room gets bigger targets.
[Touch mode](touch.md#touch-mode) still makes it taller than any of them.

```bash
ame desktop top-bar size compact
```

## At the bottom, or hidden

Both are off until you choose them, in Control's **Top bar** group or in a terminal. Notifications still appear at the
top.

- `ame desktop top-bar position bottom`: menus open upwards, and a dock at the bottom sits just above the bar.
- `ame desktop top-bar autohide on`: windows get the bar's room, and pushing the pointer against its edge brings it in,
  as over a fullscreen window.

## The clock

What the clock shows is GNOME's own choice, in Control's **Clock** group or with `ame desktop clock`: 24 or 12 hours,
seconds, the weekday and date, and week numbers in the calendar. A second time zone, for family or colleagues abroad,
shows after the clock with its city.

```bash
ame desktop clock format 12h
ame desktop clock seconds on       # the clock then changes every second
ame desktop clock week-numbers on
ame desktop clock zone Asia/Tokyo  # "Tokyo 06:41" after the clock; off takes it away
```

A zone is an area and a city from the computer's own time-zone data, such as `Europe/Paris` or `America/New_York`;
Control offers a city in each part of the world.

## The Amethystora menu

`Super+Alt+Space` opens one menu for things otherwise scattered over Settings, the terminal and
[`ame`](terminal.md#ame):

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

Type to filter or move with the arrow keys, `Enter` to choose; `Esc` leaves the filter, and a second `Esc` goes back.
From a terminal, `amethystora-menu` opens it, and `amethystora-menu theme`, `background`, `keys`, `webapps` or `system`
opens that part of it.
