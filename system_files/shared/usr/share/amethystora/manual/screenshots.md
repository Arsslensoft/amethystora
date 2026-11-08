# Screenshots

`Print` opens GNOME's screenshot bar. Amethystora adds tools to draw on the picture, a thumbnail once it is taken,
pictures pinned on top, and ways to read, hide or translate what the picture holds.

### Drawing on a screenshot

Above the bar, for an area or the whole screen, are tools to draw on the picture before it is saved: move a mark, a
pen, a box, a filled box to hide something, a highlighter, an arrow, text and numbered markers, in eight colours and
the width the slider (or the mouse wheel) sets.

| Key | Does |
| --- | --- |
| `1` to `9` | Pick a tool |
| `Ctrl+Z` | Undo |
| `Delete` | Remove the mark you picked up |
| `Ctrl+C` | Copy the picture without saving it |
| `Ctrl+S` | Save it where you choose |

### The thumbnail

Once it is taken, a thumbnail rests at the bottom right for six seconds, longer while the pointer is on it. Click the
picture to open it in your image viewer, or use its buttons: **Edit** opens it in Gradia, **Copy** copies it again,
**Pin on top** pins it (see [Pinned pictures](#pinned-pictures)), **Show in Files** shows it in its folder and
**Delete** moves it to the trash. `ame desktop screenshot thumbnail off` (or Control, **Desktop**, **Screenshots**)
puts GNOME's own notification back.

## Hide what's private

Before you send a screenshot, **Hide what's private**, the button beside the pointer button in the screenshot bar, has
the picture read and draws a solid black box over each e-mail address, card number, phone number and bank account
number it finds, and says what it covered. A solid box cannot be undone in the picture, as a blur sometimes can.

![Hide what's private](media/hide-private.webm)

- **Uncover** takes the boxes off, and **Copy** copies the picture with them; any other way of taking the picture keeps
  them too. Each box is a mark you can move or remove like the others.
- A picture asked for while it is still being read is not taken, and the card says so. In a window or a recording,
  where boxes cannot be burnt in, the button is off.
- **It will miss things**: a name, a street address, small or unusual text, or a number written in another script. Look
  at the picture before you send it.
- It is read on this computer, only when you press the button, and only what the screenshot holds: nothing is sent or
  written.

`ame desktop screenshot private off` takes the button away.

## Read it in your language

`ame desktop translate` is off until you choose it. Then the screenshot bar has a button that reads what your box holds
and translates it, on this computer, into a card with **Copy** and **Translate more**: for anything on the screen, in
any app, picture or paused video. Nothing you translate is written or sent, and it is gone with the card.

### Setting it up

The translator is not part of the image, since building it into every image would cost every update twenty minutes.
The first time, it **builds the translator on your computer** from its published source, in a Fedora container that is
removed afterwards: about 20 minutes of up to four cores and a few hundred megabytes of downloads, which it says first
and asks about. Then it asks which language you read from and which you read in, says what it downloads, and fetches it
once.

Each language's model comes from Mozilla's servers, and the data to read its script from the Tesseract project (on
GitHub), each file checked against the SHA-256 in the image.

### What it cannot do

- Machine translation can be wrong, and small or unusual text can be misread first: do not lean on it for what matters
  most.
- A box takes about 4,000 characters, and the card says when it held more.
- Chinese, Japanese and Korean are not offered: their models hold more memory than this feature allows itself.
- It cannot tell what language a box is in, so you pick.

```bash
ame desktop translate languages    # what can be chosen
ame desktop translate off          # takes the button away
ame desktop translate remove       # deletes the models and the reading data
ame desktop translate remove all   # the translator too (building it again takes the 20 minutes)
```

## Pinned pictures

A pinned picture stays above every window, on every workspace, while you work in the window below: numbers from a
chart, a recipe, a design to match. Pin one from the thumbnail, or straight from the screenshot bar: frame an area and
click **Pin on top** beside the pointer button (or press `Ctrl+P`), and it stays exactly where it was, without being
saved or copied.

![Pin a screenshot on top](media/screenshot-pin.webm)

- Drag it to move it; scroll over it to make it larger or smaller; `Ctrl` and scroll make it see-through.
- The arrow in its corner lets clicks pass through it to the window below, for tracing; its ring turns amber. Click
  the arrow again to take it back.
- The cross, or `Esc` after clicking it, lets it go. Right-click it to copy it, save it, or pick **Solid**,
  **See-through** or **Faint**.
- Up to six at a time; a seventh lets the oldest go. They are kept while the screen is locked and gone when you log
  out.
- While you share or record the whole screen, pinned pictures and the thumbnail are blurred, for everyone and for you,
  and carry a small mark: a picture of another window could hold what the share should not show. They are sharp again
  when the share ends.

`ame desktop screenshot pin off` turns pinning off and lets go of every pinned picture.

## Trim a recording

With **Trim a recording** on (Control, **Desktop**, **Screenshots**, or `ame desktop screenshot trim on`), the notice
that a screen recording was saved offers **Trim**. It opens the recording as a strip of frames: drag its two ends (or
use the **Start** and **End** sliders), and the frame at the end you move shows above, with the times under the strip.

**Save as WebM** keeps that part as a video, and **Save as GIF** as a short GIF for a chat or an issue, both beside the
recording, as `NAME (trimmed).webm` or `.gif`; **Show in Files** shows it. The recording itself is never changed and
nothing is written over. To trim any recording from a terminal:

    amethystora-trim ~/Videos/Screencasts/recording.webm

## Copying text from the screen

Text in a picture, a video call, an error dialog that cannot be selected, a scanned page: Gradia reads it and copies it
for you, on your machine.

![Copy text from the screen](media/extract-text.webm)

1. Press `Print` and drag over the text.
2. Click **Extract Text** in the screenshot bar, or press `Ctrl+E`.
3. The first time, Gradia asks for a language. Pick it: this is the one download, of that language's data from GitHub,
   and it is yours to agree to. After it, nothing leaves the machine.
4. The text is copied at once, and Gradia says **Copied!**. Paste it anywhere; it is in the
   [clipboard history](clipboard.md#the-clipboard) like anything else you copy.

Good to know:

- The button and `Ctrl+E` work on an area only. They are off when you choose a window, the whole screen or a recording.
- The capture is saved like any screenshot, in your screenshot folder. Delete it there if you do not want it.
- The language data lives in `~/.var/app/be.alexandervanhee.gradia/data/tessdata`. Open Gradia's preferences to add
  another language.
- Gradia is one of the apps every machine comes with; if you removed it, install it again from Bazaar and the button
  comes back.
