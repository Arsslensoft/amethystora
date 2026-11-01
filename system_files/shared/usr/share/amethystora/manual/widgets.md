# Widgets

The top bar can show things of your own: a focus timer, the weather, unread mail, whether the VPN is
up, how many pull requests wait for your review. Each one is a *widget*, a small program whose output
the bar turns into a label, an icon and a menu. You do not have to write it yourself: the
[AI agent](ai-agent.md) does, once you turn it on (`ame agent toggle on`).

## Ask the agent for one

Choose **Make a widget** in the Amethystora menu (`Super+Alt+Space`) and say what it should show, or
run:

```bash
amethystora-widgets make "a pomodoro timer"
amethystora-widgets make "how many pull requests wait for my review on GitHub"
amethystora-widgets make                          # the agent asks you what you want
```

The agent writes the widget, checks it, and it appears in the bar as soon as it is saved, with no
need to log out. Ask for changes the same way: "make the weather widget show tomorrow too".

When a widget stops working, the bar shows a warning sign in its place. Click it to read what went
wrong; **Fix it with the AI agent** hands the problem to the agent.

## The gallery

Some widgets come ready, and [Control](control.md)'s **Desktop** page, under **Top bar widgets**, adds each in one
click, to the bar or to the desktop: a **Clock** in big type with the day under it, a **Calendar** with the date and the
week's number, **Now playing** with the player's buttons, **System** with how busy the processor is, the memory and the
disk, the **Pomodoro** timer and **Do not disturb**. None is on until you add it, and each is a widget like any other:
a folder of your own you can read and change. Adding one to the desktop turns widgets on the desktop on too. In a
terminal:

```bash
amethystora-widgets add clock desktop
amethystora-widgets add system bar
```

## Commands

| Command | Does |
| --- | --- |
| `amethystora-widgets` | Lists your widgets, and which are on |
| `amethystora-widgets make ["<idea>"]` | Has the agent make one |
| `amethystora-widgets new <name> [<example>]` | Starts one from an example: `hello`, `pomodoro`, `do-not-disturb`, `clock`, `calendar`, `now-playing` or `system` |
| `amethystora-widgets add <example> [bar\|desktop]` | Adds a widget of the gallery under a name of its own, to the bar or the desktop |
| `amethystora-widgets run <name>` | Runs it once the way the bar does, and says what is wrong with it |
| `amethystora-widgets fix <name> ["<what is wrong>"]` | Has the agent find out why it fails, and fix it |
| `amethystora-widgets off <name>` and `on <name>` | Hides it from the bar, or shows it again |
| `amethystora-widgets remove <name>` | Moves it to the trash |
| `amethystora-widgets restart` | Stops and starts every widget, and turns the widgets extension back on if it was off |

`ame desktop widgets` runs the same commands. [Control](control.md)'s **Desktop** page lists your
widgets, turns each on or off, starts one from an example, and has the agent make or fix one. A
widget's name is lower-case letters, digits and dashes, up to 40 characters.

## Making one yourself

A widget is a folder in `~/.config/amethystora/widgets`, holding a `widget.json` that names a command
and says how often to run it, and the command itself, usually a short script. The script prints a
line for the bar to show: plain text, or JSON with an icon, a colour from the theme and a menu whose
items run commands, open links or flip switches.

`amethystora-widgets new mywidget` copies the `hello` example into place to start from, and
`amethystora-widgets run mywidget` checks it after every change. The full format is the agent's own
guide to it, `/usr/share/amethystora/agents/skills/amethystora-widgets/SKILL.md`; the examples are in
`/usr/share/amethystora/widgets/examples`.

## On the desktop

A widget can be a small card on your wallpaper instead of a button in the bar: a clock, a build's status, the
day's tasks. Turn widgets on the desktop on in Control (**Desktop**, **Widgets on the desktop**) or with
`ame desktop desktop-widgets on`, then add to its `widget.json`:

```json
{"name": "Clock", "command": "./clock.sh", "interval": 60, "place": "desktop", "position": "top-left", "size": "large"}
```

- `place` is `desktop` (the bar is the default, `bar`).
- `position` is the corner of the screen: `top-left`, `top-right` (the default), `bottom-left` or
  `bottom-right`. Several widgets in one corner stack, in their `order`.
- `size` is `large` for a first line in large letters, as a clock has; `normal` otherwise.

The card shows what the command prints, as the bar would: the icon beside the first line, the next lines a
little smaller (six lines at most), the colour on the last one. A click runs its `click` or opens its menu. The
cards are on the main screen, under your windows, and not in the overview, and run while the widgets extension
itself is on. A desktop widget never shows in the bar, and while widgets on the desktop are off it does not run at
all. `amethystora-widgets run` checks it the same way.

## Safety

A widget runs as you, with access to your files, like any program you start. Only keep widgets you
have read or asked the agent for, and read what the agent proposes before you say yes, as it asks
before it writes anything.

Widgets run outside GNOME Shell, so a broken or slow one shows a warning sign instead of freezing the
desktop or ending your session. They stop while the screen is locked.
