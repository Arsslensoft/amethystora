# Clipboard, shelf and emoji

What you copy, carry between apps, and type that is not on the keyboard: the clipboard history, the shelf, emoji and
symbols, and text shortcuts.

## The clipboard

`Super+Shift+V` opens the clipboard history: the last 15 things you copied. Pick one to copy it again, type to search,
and pin the ones you keep coming back to. It is also the clipboard icon in the top bar.

![The clipboard history](media/clipboard-history.webm)

### Text actions and QR codes

The ⋯ button of a text in the history (or `A` on it) opens **Make a new copy**:
the text as **Pretty JSON**, in **UPPER CASE**, **lower case** or **Title Case**, with its spaces trimmed, on
**One line**, or with a web address's `%20`s decoded, each one a new item at the top of the history and on the
clipboard, ready to paste, and the text's characters and words counted. The original stays as it was. For a link
or a short text, **Show as QR code** shows it as a code any phone's camera reads, to hand it to a phone that is not
paired, or to somebody else's. While the screen is shared or recorded the code is not shown.

### How it is kept

The history is never on disk as you copied it: text and pictures alike are sealed (encrypted) in
`~/.local/state/amethystora/clipboard`, under a key kept in your login keyring, which your password protects. A copy
of the disk, a snapshot or a stolen laptop shows nothing of it without that password. It is left out of
[backups](backups.md). A keyring given no password, as on some machines that sign in by themselves, keeps its key in
the clear, and then the history is only as safe as the disk.

How long it is kept is yours to choose, in [Control](control.md)'s **Clipboard history** or in a terminal. Pinned items
are always kept, and the history outlasts locking the screen either way.

```bash
ame desktop clipboard always    # everything, until you clear it (the default)
ame desktop clipboard restart   # pinned items, and the rest until the computer restarts
ame desktop clipboard session   # pinned items, and the rest until you log out
```

### Secrets, and a locked keyring

Nothing is recorded of what a password manager marks secret, as KeePassXC does, or of what you copy in KeePassXC,
Secrets, Bitwarden, 1Password or Proton Pass. Before copying anything else secret, turn on **Private mode** in the same
menu, and nothing is recorded until you turn it off.

After logging in with a fingerprint or a security key, your keyring stays locked, so the menu shows what you copy from
then on, under **Unlock to see earlier items**: click it and type your password to see the rest. The history is sealed
under this machine's keyring: on another machine, or after an administrator resets your password, it does not open, and
it is kept aside as `history.unreadable`.

## The shelf

The shelf is for moving things between two apps that are never on screen together: put them down, go where they
belong, pick them up. It is off until you turn it on in Control (**The shelf**, on the Desktop page) or with
`ame desktop shelf edge`.

Start dragging a file, some text or a picture, and a glow appears at the right edge of the screen. Let go on it, and
the shelf opens there: a narrow column that stays above your windows, on every workspace, until it is empty.

![Drop on the shelf](media/shelf-catch.webm)

- Drop more onto it whenever you like. Drag an item out into any app; it leaves the shelf once the app has taken it.
  Hold **Ctrl** while you drag to keep it on the shelf as well.
- **Clear** empties the shelf, and the shelf closes when the last item goes.
- Rather not aim for the edge? `ame desktop shelf shake` opens it wherever you are: shake the pointer from side to side
  while dragging, and let go on the card that appears.
- The shelf is a window, so `Alt+Tab` reaches it. **Delete** takes the item in focus away, **Ctrl+Delete** clears the
  shelf, and a right click (or the Menu key) on an item has **Remove**, **Show in Files** for a file, and **Clear the
  shelf**.

### What the shelf holds

A file stays where it is: the shelf holds a reference to it, and dragging it out gives the app a copy, never a move,
even onto a folder in the [dock](dock.md#what-the-dock-holds). If the file is moved or deleted meanwhile, its item turns
grey.

Text and pictures are kept in memory only, by the shelf and, while the first drop is handed over, for a moment by the
desktop: neither saves them anywhere, and they never reach the [clipboard history](#the-clipboard). The shelf lets go
of them when they leave it, when it is cleared, or when you log out. If the shelf or the desktop crashed meanwhile, its
crash report could hold them; `ame security no-coredumps` keeps crash reports from being written.

The shelf takes a drop of up to 1 MB of text or of a list of files, or a PNG picture of up to 32 MB and 64 million
pixels, and holds 32 items; it says so when it cannot take something.

## Emoji and symbols

`Super+.` opens emoji and symbols over the window you are typing in, under the text cursor. Type to
search, in your language or in English (`euro`, `arrow`, `smile`), or pick a tab: **Recent**,
**Smileys**, **Arrows**, **Maths**, **Money**, **Punctuation**, and the rest of Unicode's emoji. `Enter`
or a click puts the one picked where you were typing; `Escape` closes it without typing anything. The
arrow keys move in the grid, `Tab` goes between the search, the tabs and the grid, and
`Ctrl+Page Down` and `Ctrl+Page Up` change tab.

What you pick is typed straight in where the app allows it. Otherwise it goes through the clipboard and
is pasted with `Shift+Insert`, or `Ctrl+Shift+V` in a terminal, and then it stays on the clipboard.

Hands and people come in the skin tone you choose: right-click one, hold it, or press the `Menu` key on
it for the six tones, and the one you pick becomes the tone for the rest. It is also in Control's
**Typing** group, or:

```bash
ame desktop emoji tone medium    # none, light, medium-light, medium, medium-dark or dark
```

### Where the emoji come from

Your recent picks are kept on this computer only, in `~/.local/state/amethystora/emoji.json`. The lists
and the names come from Unicode's emoji list (Emoji 16.0) and the Unicode CLDR, under the Unicode
License. Smile, the emoji app, is still on `Ctrl+Alt+Space`. IBus's own emoji panel has no shortcut, so that
`Super+.` reaches the picker in every text field.

## Text shortcuts

A text shortcut turns a few characters into a longer text as you type: `;addr` and a space becomes your
address, `;sig` your signature, `:shrug` becomes ¯\\\_(ツ)\_/¯. Add them in Control's **Typing** group,
which also has a field to try them in, and turn on **Expand text shortcuts** there, or in a terminal:

```bash
ame desktop text-shortcut add ';addr'    # then type the text, and Ctrl+D on a line of its own
ame desktop text-shortcuts on
```

`ame desktop text-shortcut list` lists your shortcuts, `ame desktop text-shortcut remove ';addr'` takes
one away, and adding a shortcut you have already gives it the new text (Control asks first). You can
also give the text after the shortcut, `ame desktop text-shortcut add ';addr' '4 Amethyst Lane'`, but then
every account on the computer can see it in the list of running programs while the command runs; typed
or piped in, and from Control, it never is.

Type the shortcut where a word starts, then a space. Start shortcuts with a character you seldom type before a word,
like `;` or `:`, so that they never expand by accident. Write `{date}` in a text for today's date. Your app's Undo
takes an expansion back. A shortcut is 2 to 32 characters with no spaces, a text up to 2000 characters, and you can
have 200.

### What it reads while you type

It is off until you turn it on. While it is on, the desktop looks at what you type in apps for your
shortcuts: it keeps only the last few characters, as many as your longest shortcut, in memory, and
never writes them anywhere. It forgets them when you switch windows, when a field of another kind (a
password field, say) or one of the desktop's own takes the keyboard, and at every key that is not text,
such as an arrow key, Enter or Tab. It cannot tell when you click into another field of the same window,
so it keeps them then until one of those happens. Outside terminals it only replaces a shortcut it can see
the app holds, so what you typed in one field is never replaced in another. It never looks at password or PIN
fields, at fields for numbers, phone numbers or dates, at the desktop's own fields, or at what an input
method like Pinyin or Mozc is composing. A key you press while a shortcut is being replaced stops the
replacing. Your input sources and input methods stay as they are.

### Where it works

It works in apps that use GNOME's input method and tell it the text they hold: GNOME's apps,
Amethystora's, Firefox and most others. In terminals, which do not tell it, a shortcut is replaced with
BackSpace keys, and only one whose text has a single line, since a new line would run the command; if you
paste into a shortcut halfway, or finish one in another pane of the same terminal window, what you pasted
or typed there can be taken for part of it. Other apps that do not tell
it their text get no expansion, since there a click could have moved the cursor unseen. It does not work
in apps that run through X11 (older apps, games, Wine) or that talk to IBus themselves.
The shortcuts are in `~/.config/amethystora/text-shortcuts.json`, and only you can read it. They are not
part of your setup file, since they are your own writing.
