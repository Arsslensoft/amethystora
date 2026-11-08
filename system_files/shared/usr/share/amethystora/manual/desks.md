# Desks

A laptop lives at several desks. GNOME already remembers how the screens were arranged at each; a desk remembers the
rest: the sound output, where the dock is, a [Focus](focus.md#focus) profile and the power mode. Desks are off until
you turn them on in Control (**Desks**, on the Desktop page) or with `ame desktop desks on`.

### How a desk is known

By its screens: their maker, model and serial number, as the screens report them. Two identical screens without
serial numbers are told apart by the socket they are plugged into, and two desks with the same screens by their
Thunderbolt docks; a Thunderbolt drive plugged in leaves it the same desk. The computer's own screen does not count, so
a desk is the same with the lid open or shut, and unlocking the screen at the same desk is no arrival.

### The first time at a desk

A card at the top right shows how the four things are set now, and **Set up** remembers them for that desk (Focus only
if a profile is on). **Not now** is remembered: that desk does not ask again until `ame desktop desks ask`. Left
unanswered, the card asks at the next arrival.

### Arriving and leaving

Arriving at a known desk sets what differs at once, and a toast at the bottom says what it set, with **Undo**. When
you unplug, whatever the desk set and is still as it set it goes back to how it was before; something you changed
yourself meanwhile stays.

### Learning, and saved sets

- With **Learn from what I change here** on (it is, with Desks), picking another sound output or moving the dock at a
  desk asks, once for each change, whether the desk should remember it. A sound output that just appeared, like
  headphones connecting, is not asked about.
- A desk's power mode is set only while no Focus profile or game holds one, and a held mode is never taken for the
  desk's own.
- A desk can offer one of your [saved sets](workspaces.md#saved-sets) of windows: choose it in Control, or
  `ame desktop desks saved ID NAME`; arriving then asks before opening it.
- A desk never changes a security setting and never starts an app by itself.

### Managing desks

```bash
ame desktop desks list                  # the desks and what each sets
ame desktop desks rename ID Office desk # until then a desk is named after what it has, like "2 screens"
ame desktop desks forget ID
```

The desks are kept in `~/.config/amethystora/desks.json`, which only your account can read. The screens' serial
numbers stay there: they are never in [your setup file](setup.md), which carries only whether Desks are on.

## Screens come back as you left them

Plug the dock back in and every window goes back to its screen, place and size, instead of a pile on one display. It
needs no desk set up, and it is on (Control's **Screens come back as you left them**, or
`ame desktop desks windows on|off`).

- In the tiling, each workspace's windows go back to their columns, in their order and at their widths, on the screen
  that showed them. In the classic layout each window goes back to its place, which needs
  [windows that go back to their places](classic-layout.md).
- A set of screens is known by maker, model and serial number, so a dock that names its ports differently today
  changes nothing.
- Windows are only moved: nothing is started or closed, and a window you opened since stays where it is. A floating
  window under the tiling, and the heights in a stacked column, are not kept.
- Only which app it was, its place and a hash of the screen are kept, in `desks.json`, for the six sets of screens
  seen last: never a window's title, a file or a page. Turning it off forgets them.
