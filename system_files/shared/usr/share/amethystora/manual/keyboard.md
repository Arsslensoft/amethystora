# The keyboard

Control's **Keyboard** page (`amethystora-control keyboard`) has every keyboard shortcut, and the keyboard settings
GNOME's Settings does not show.

## Every shortcut

The **Shortcuts** card lists GNOME's shortcuts, the desktop's, the tiling's hundred and your own custom ones in one
list, with a field to find one by what it does or by its keys. Each says whose it is, or **Yours** once you changed it.

- Two shortcuts on the same keys are marked in red with the other one's name, unless both are as Amethystora ships
  them: the tiling takes some of GNOME's keys while it is on, on purpose. A tiling shortcut works only while the
  tiling is on.
- **Change** waits for the new keys: press them, `Escape` to give up, or `Backspace` to turn the shortcut off. So that
  keys the desktop already uses reach it, Control holds the desktop's own shortcuts back while it listens; GNOME asks
  you once whether it may. **Reset** puts back the keys it came with.
- A shortcut needs `Super`, `Ctrl` or `Alt`, unless it is a function key, `Print` or a media key, so that no key stops
  typing.

What you set is written where GNOME's Settings and the tiling's own window read it, and your [setup file](setup.md)
keeps it. The AI agent may never change the shortcuts that open it.

```bash
ame desktop shortcut list
ame desktop shortcut set org.gnome.settings-daemon.plugins.media-keys previous '<Super>Left'
ame desktop shortcut clear org.gnome.settings-daemon.plugins.media-keys previous
ame desktop shortcut reset org.gnome.settings-daemon.plugins.media-keys previous
ame desktop shortcut set custom custom20 '<Super>t'    # a custom shortcut: custom and its id
```

## Keyboard settings

| Setting | What it does | In a terminal |
| --- | --- | --- |
| **Caps Lock acts as** | Escape, Ctrl, Backspace, nothing, or Caps Lock | `ame desktop keyboard caps escape` |
| **Compose key** | Right Alt, Right Ctrl, Menu, Right Super or Caps Lock starts a composed character: Compose, then `'` and `e`, types é | `ame desktop keyboard compose ralt` |
| **Super on its own opens** | The overview, the [search box](search.md), or nothing; Super's shortcuts work either way | `ame desktop keyboard super search` |
| **Middle click pastes** | The middle button pastes the last text selected, anywhere | `ame desktop keyboard middle-paste off` |
| **Num Lock on at start** | On at every start, rather than as it was left | `ame desktop keyboard numlock on` |
| **Repeat after** | How long a key is held before it repeats, 150 to 1000 ms | `ame desktop keyboard repeat-delay 400` |
| **Each window keeps its keyboard layout** | See [below](#a-layout-for-each-window) | `ame desktop keyboard per-window on` |

`ame desktop keyboard` alone says how each is set. The Caps Lock and Compose settings change only their own part of
GNOME's keyboard options and keep the rest. All of them are in your setup file.

## A layout for each window

With more than one keyboard layout (GNOME's Settings, **Keyboard**), each window can keep its own: Arabic in a chat,
French in work mail, English in the terminal, each coming back when you return to that window.

![A layout per window](media/layout-per-window.webm)

Turn on **Each window keeps its keyboard layout** on the Keyboard page, or `ame desktop keyboard per-window on`.
`Super+Shift+Space` still switches the layout of the window in use.

## Paste as plain text

`Super+Alt+V` pastes what is copied without its fonts, colours and links, into any app, whatever its own paste does;
`Ctrl+V` pastes as before. The text is typed in where the app allows it; otherwise it is put back on the clipboard as
plain text and pasted. A password that a password manager marks secret is never put back on the clipboard, so the
clipboard's history still keeps no copy of it. Change or turn off the key on the Shortcuts card.

## Hold a key for accents

Hold a letter and its accented forms appear in a strip over the text cursor: hold `e` for é è ê ë ē ė ę. Press `1` to
`8`, or the arrow keys and `Enter`, to type one in place of the letter; `Escape`, or any other key, leaves the letter
as it is. For writing French, Spanish, German, Polish or Turkish on an English keyboard.

![Hold a key for accents](media/hold-accents.webm)

- Off until you turn it on: **Hold a key for accents** on the Keyboard page, or `ame desktop accents on`. While it is
  on, a held letter no longer repeats in the apps it works in.
- Like text shortcuts, it looks at the keys you type in apps, keeps nothing of them but the letter held, and never
  looks at password fields.
- It works in apps that tell GNOME's input method their text (GNOME's apps, Amethystora's, Firefox and most others),
  not in terminals, fullscreen windows such as games, or apps that run through X11.
- It is not part of your setup file.
