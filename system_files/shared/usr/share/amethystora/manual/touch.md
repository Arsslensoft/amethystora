# Touch and pen

For a computer you touch or draw on, Control's **Desktop** page has a **Touch and pen** group, and `ame desktop touch`
alone shows how each is set. Both are off until you choose them.

## Touch mode

The desktop sized for a fingertip: the top bar 12 px taller with larger text, the dock's icons up to 64 px, menus with
rows at least 44 px high, and **Today from the right edge**: a handle there, and a swipe in from that edge opens the
calendar menu. Apps keep their own sizes.

![Touch mode](media/touch-mode.webm)

- `ame desktop touch mode follow` turns it on while the computer is folded into a tablet: Mutter tells by the hinge's
  tablet switch, or, where there is no switch, by a touch screen with no mouse or touchpad. GNOME's on-screen keyboard
  follows the same signal by itself.
- `ame desktop touch mode on` keeps it on; `off` turns it off.

While it is chosen, quick settings has a **Touch mode** tile. Some hinges report the wrong thing: a click on the tile
changes it until the hinge next moves (or you log out), without changing your choice.

## The pen ring

Six tools at the pen's tip, each run where the ring was opened: **Scribble** (drawing on the screen, where present mode
offers it), **Area** (a screenshot of an area), **Note** (a quick note into Notes), **Text** (the text in an area,
copied, with Gradia), **Right click** and **Back**. Move the pen toward a tool to light it, then touch the screen, or
carry on past the ring's edge. A touch in the middle, `Escape` or a click outside closes it.

![The pen ring](media/pen-ring.webm)

- `ame desktop touch pen-ring ring` turns the ring on; `strip` shows the tools in a column at the screen's edge
  instead; `off` turns them off. `Super+Ctrl+P` opens them.
- `ame desktop touch pen-tools area note text right-click back` picks the tools and their order, from the top,
  clockwise.

### The pen's button

The pen's side button opens the tools once it sends that shortcut. `ame desktop touch pen-button on` sets it up, keeping
what each button did, for each pen whose settings GNOME has or that has touched the tools, and for pens without a
serial number of their own on the tablets attached now. `ame desktop touch pen-button off` gives that back, where the
button still does what it set. The command says which pens it set.

A pen with a serial number of its own (most Wacom pens) is known only once it has been used: open the tools with
`Super+Ctrl+P`, touch them with the pen and run the command again, or choose `Super+Ctrl+P` for its button in Settings,
under **Wacom Tablet**.
