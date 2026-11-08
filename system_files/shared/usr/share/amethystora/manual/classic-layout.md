# Windows in the classic layout

What follows is for the classic layout, unless it says otherwise. `ame desktop windows` lists each of these
settings and how it is set now, and [Control](control.md)'s **Desktop** page has them under **Windows**,
**Hot corners** and **Touchpad gestures**.

## Snap layouts

![Snap layouts](media/snap-layouts.webm)

Drag a window by its title bar to the top of the screen, or press `Super+Z`, and six layouts appear:
halves, two thirds and a third, thirds, quarters, the middle, and the whole screen. Move over a part of one
and the screen shows where the window will go; let go, or click, and it goes there. With `Super+Z` the
arrow keys move between the parts and `Enter` chooses. Dragging it away gives it its old size back. The top
edge itself still maximises, and `Super` and the arrows still work. Off with `ame desktop windows snap off`.

## Switching windows

![Switch windows](media/super-tab.webm)

`Super+Tab` and `Alt+Tab` show every window in a grid, the windows themselves, live. Keep the key down and
press `Tab` to move on, or start typing: only the windows whose app or title has those letters stay, and
the grid stays open after you let go, until `Enter` switches or `Esc` leaves everything as it was. In the
tiling layout the keys switch as GNOME does. `ame desktop windows switcher off` gives GNOME's switcher back.

## Holding Super

![Hold Super](media/hold-super.webm)

Hold `Super` for a second, without another key, and a card shows the common shortcuts, as they are set on
this machine, your own changes included, with numbers on the dock: `Alt` and a number opens the app in that place.
Let go and they fade. A quick press of `Super` still opens the overview. [Control](control.md)'s Keyboard page has
every shortcut. `ame desktop windows hold-super 1500` waits a
second and a half; `off` turns it off. This works in both layouts.

## Hot corners

Each corner of the screen can do one thing when you push the pointer into it: the overview, show the
desktop, lock the screen, a quick note, or the notifications. All four do nothing until you choose, in
Control or with `ame desktop windows corner top-right overview`. A corner fires after a short push past
the edge, so passing by never sets it off. Corners next to another screen have none, and GNOME's own hot
corner, if you turn it on in Settings, keeps the top left. The corners work in both layouts; show the
desktop only in the classic one.

## Show desktop

With `ame desktop windows show-desktop on`, clicking empty wallpaper slides every window to the edges,
leaving a sliver of each. A click anywhere, or any key, brings them back; clicking a sliver also brings
that window to the front.

## Touchpad gestures

![Pinch for the apps](media/pinch-apps.webm) ![Spread for the desktop](media/spread-desktop.webm) ![Four fingers for Today](media/four-left-today.webm)

| Fingers | Does |
| --- | --- |
| Three up | GNOME's overview; again for the app grid |
| Three sideways | GNOME's next workspace |
| Pinch with three | The app grid |
| Spread three | Show the desktop; pinch to bring the windows back |
| Three down | The windows of the app you are in, fanned out from the dock |
| Four to the left | Today and the notifications; to the right closes them |

Two fingers stay the app's own. Each is a switch: `ame desktop windows gesture pinch off`. The pinch and
four fingers work in both layouts, unless the tiling's own gestures use four fingers.

## Places

Windows go back to their places for each arrangement of screens: plug a screen in and each app's windows go
where they were the last time it was there; unplug it and they return to their laptop places. Apps cannot
place themselves on Wayland, so Amethystora keeps the places, by the screens' connectors and sizes, in
`~/.local/state/amethystora/places.json`. `ame desktop windows places forget` forgets them.
