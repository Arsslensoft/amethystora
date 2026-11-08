# Getting around

The desktop is GNOME, arranged for the keyboard. Everything also works with the mouse, but once the keys are in your
fingers you will rarely reach for it.

## Show me, on my screen

Where the manual has **Show me**, it rings the real button on your screen and walks you through it, a step at a time:
**Next** when you are ready, or the step moves on when you do what it says. It presses nothing for you, and a step it
cannot find says its words and leads back to the page. The manual steps aside while a guide runs and comes back however
it ends. Turn it off in Control (**Desktop**, **Help**) or with `ame desktop show-me off`; the manual's words stay.

## Gestures and keys

Each card moves the way the gesture or the keys do. Click one, or press `Space` on it, to pause it; with animations
off they stay still. Shake to find is off until you turn it on ([Pointers](styles.md#pointers)).

![Shake to find](media/shake-to-find.webm) ![Pinch for the apps](media/pinch-apps.webm) ![Search anything](media/super-space.webm) ![Hold Super](media/hold-super.webm)
![Switch windows](media/super-tab.webm) ![Snap layouts](media/snap-layouts.webm) ![Spread for the desktop](media/spread-desktop.webm) ![Four fingers for Today](media/four-left-today.webm)

## Tips

The first time a feature can help, a small card beside it says how, with **Got it**: shaking the mouse to find the
pointer, holding `Super`, `Super+Space`, snap layouts, Files' Scripts menu, sending a file to your phone, and the pinch
on the touchpad. Each tip comes once, ever.

- None in your first hour, never two at once, and at least half an hour apart.
- None while a window is fullscreen, the screen is shared or recorded, notifications are silenced, or over the
  overview or a dialog.
- A tip never takes your keyboard or clicks, and closes by itself after twelve seconds.

```bash
ame desktop hints off      # no tips; on brings them back
ame desktop hints reset    # the tips you have seen can come again
```

## Extensions

These GNOME Shell extensions come with the image:

| Extension | What it does |
| --- | --- |
| Amethystora Clipboard | The [clipboard history](clipboard.md#the-clipboard) on `Super+Shift+V`, sealed: a copy of Clipboard Indicator |
| Amethystora UI | Amethystora's own desktop: [tiling windows](tiling.md) on a scrolling strip, the dock, the glass (Frost, Prism or Clear, see [Glass](styles.md#glass)) behind the panel, overview, menus, notifications and windows you are not using, the workspaces, meters and tray icons in the top bar, a phone icon while a [paired phone](wireless.md#phones) is connected, the search on `Super+Space` and app search that forgives typos, drawing on a screenshot and [copying the text out of it](screenshots.md#copying-text-from-the-screen), uninstalling an app from its menu, quicker animations, no workspace popup and no Activities button; the [live pill](pill.md), [privacy dots](notifications.md#privacy-dots), [quiet notifications](notifications.md#quiet), the volume pill and your [widgets](widgets.md) |
| Logo menu | The Amethystora logo at the top left and [its menu](top-bar.md) of the system's tools |
| Amethystora Transitions | The animation that reveals a new theme or wallpaper ([Transitions](wallpapers.md#transitions)) |

Turn any of them off, or change their settings, in **Extensions**. Extension Manager is for finding new ones on
extensions.gnome.org, which has none of these: its **See Details** says "An Error Occurred: No extension details
available" for them, and nothing is wrong. Each one's description and version are in Extension Manager's row, under the
arrow, and in **Extensions**, under the three dots.

After every image update the image's own extensions are switched back on, so that one turned off by accident or by a
crash does not stay off for good: turning one off lasts until the next update. The
[classic layout](tiling.md#classic-windows) is not an extension turned off, so it stays as you set it.
