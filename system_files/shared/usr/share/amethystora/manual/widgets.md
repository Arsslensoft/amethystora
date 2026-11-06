# Widgets

A widget is a plugin that shows in the top bar or on the desktop. The widgets of earlier images, small programs that ran with everything you
had, are **plugins** now: each runs in a sandbox of its own and reaches only what you allowed, and what you add is read by you first. Everything
about them is on the [plugins](plugins.md) page; this one is what changed for widgets.

## Ask the agent for one

Choose **Make a widget** in the Amethystora menu (`Super+Alt+Space`) and say what it should show, or run:

```bash
ame plugin make "a pomodoro timer"
ame plugin make "how many pull requests wait for my review on GitHub"
```

It appears as a draft with nothing allowed; you keep it, change it or throw it away. See [made by the agent](plugins.md#made-by-the-agent).

## The gallery

Some come ready, and [Control](control.md)'s **Desktop** page lists them under **Ready to add**: a **Clock** in big type with the day under it, a
**Calendar** with the date and the week's number, **Now playing** with the player's buttons, **System** with how busy the processor is, the memory and
the disk, a **Pomodoro** timer in the pill and **Do not disturb**. None is on until you add it, and adding one shows you what it reaches first:

```bash
ame plugin shipped
ame plugin add amethystora.clock
```

`amethystora-widgets` and `ame desktop widgets` still work as other names for `ame plugin`. Nothing is read from `~/.config/amethystora/widgets`
any more: a folder left there runs nothing.

## On the desktop

A plugin that asks for the desktop is a small card on your wallpaper, in the corner it names. Turn plugins on the desktop on in Control
(**Desktop**, **Plugins on the desktop**) or with `ame desktop desktop-widgets on`. The cards are on the main screen, under your windows, and not in the
overview.

## Safety

A widget cannot read your files, reach the network or see your other programs unless you allowed it, and it stops while the screen is locked. A
broken or slow one is paused and says so instead of freezing the desktop.
