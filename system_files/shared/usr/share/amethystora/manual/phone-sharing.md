# Sharing with your phone

Notifications, files, links, photos and the clipboard between a paired [phone](phone.md) and this computer. Nothing
from the phone opens by itself.

## Notifications

While the phone is connected, its notifications appear here as ordinary desktop notifications. Ones it already had
when it connected are not shown again, nor ongoing ones such as a player's or a download's progress. What a
notification offers depends on the phone's app:

- **Reply**, for a message you can answer from the phone's notification: it opens a small window with the
  text and a box to answer in.
- The buttons the phone's own notification has, such as **Archive** or **Mark as read**, which do that on the
  phone.
- **Dismiss**, for one the phone lets you clear. Closing a notification here dismisses it on the phone, and
  dismissing it on the phone takes it away here.

Whatever the phone says is shown as text, never as a command: a link in a notification is not opened by
itself.

## Files and links

### Sending

Select a file in Files and choose **Open With → Send to Phone** (offered for pictures, videos, sounds, PDFs, text files
and archives, never a file's default), press **Send…** on a phone's card (any file), or use the phone icon in the top
bar. A window asks which phone, if more than one is connected, and shows each file's progress.

![Sending a file to the phone](media/phone-send-file.webm)

Folders cannot be sent, only the files in them. A link goes from the same window's **A link** tab, or from the top bar
with the link on the clipboard; the phone opens it with whatever handles it.

```bash
amethystora-phone send --device ID report.pdf photo.jpg
amethystora-phone send https://example.org/menu
```

### Receiving

A picture goes to [Pictures](#photos), any other file to `~/Downloads/Phone/` in a folder named for the phone. Nothing
is overwritten: the second `report.pdf` is `report (1).pdf`. The notification offers **Show in folder**, and **Open**
for a picture, a text, a PDF, a sound or a video only; nothing opens by itself.

Text or a link the phone shares comes as a notification whose **Show** opens a window with the words as text, to
select and copy, with web addresses you can click. A web link also waits in the [live pill](pill.md) until you open or
copy it; **Not now** leaves it in the notification list, with **Open** and **Copy**.

## Send to nearby

Select files in Files and choose **Scripts → Send to nearby**, or drop them on Phone in the dock: the send window shows
the paired phones and computers on a network you named home as round avatars. Pick one and press **Send**. Nothing
goes to a device that is not paired or is away.

### Files that wait

A file from **another computer** waits: a card says who sends what, with **Accept** and **Decline**. Accepting covers
the rest of what the sender announced, for ten minutes; declining, closing the card or leaving it a minute tells the
sender to stand down. A paired phone sends as before, since it is the one in your hand. To change whose files wait:

```bash
ame desktop phone ask-first computers   # the default: another computer's files wait
ame desktop phone ask-first all         # phones' too
ame desktop phone ask-first never       # every file arrives at once
```

or **Files wait for Accept from** on [Control](control.md)'s **Desktop** page. A file never overwrites one that is there, and lands in
`~/Downloads/Phone/`, as [above](#files-and-links).

## Photos

A picture the phone shares, from its gallery or camera, goes to `~/Pictures/Phone`, with every phone's pictures
together. A picture is a file ending in `.jpg`, `.jpeg`, `.png`, `.heic`, `.heif`, `.webp`, `.gif`, `.avif` or
`.dng`; a video goes with the other files. The name is the phone's, made safe, and a second `holiday.jpg` is
`holiday (1).jpg`. The notification has **Open** and **Show in folder**; nothing opens by itself.

Like any picture in Pictures, it is read by GNOME's search (its picture reader runs in a sandbox of its own), so you
can find it by name in Files and the overview; pictures kept in Downloads are not.

To keep pictures with the phone's other files in Downloads instead:

```bash
ame desktop phone photos downloads   # or pictures, to put them back in Pictures/Phone
```

or **Photos from the phone go to** on [Control](control.md)'s **Desktop** page.

To add a photo, take it on the phone and share it to this computer from the phone's share menu; it arrives like any picture.
There is no button for this in the KDE Connect app any more, so this computer cannot make a current phone open its camera: the
phone link says only what it has established. See [Snap with your phone](#snap-with-your-phone) for the one case where there is.

## Snap with your phone

KDE Connect once had a request that opened the phone's camera and sent the picture back, and removed it from the phone's
app in 2023. Only a phone that still lists it (an old install, or a fork) can be asked. It is **off**:

```bash
ame desktop phone photo on    # or off
```

or **Snap with your phone** on [Control](control.md)'s **Desktop** page. On, a phone that takes the request has a **Snap
with your phone** button on its card in the Phone app; other phones have nothing, and the page says to take the picture
on the phone and share it here.

The button asks the phone to open its camera (this computer cannot know that it did, and says only that it asked). A
notification waits for the picture with **Cancel**, which only forgets the request here: it cannot close the camera on
the phone. One request waits per phone, for two minutes; a picture nobody asked for is refused, and so is one over
64 MiB. It lands where the phone's other pictures go ([Photos](#photos)), never over a file, and nothing opens by
itself. Only that button can open a phone's camera: the agent and other programs of your account cannot.

## The clipboard

The clipboard is shared in two directions, each its own switch, both **off**: a phone that can put text on this
clipboard can put it where you will paste it into a password field.

```bash
ame desktop phone clipboard send on      # what you copy here goes to the phones you paired
ame desktop phone clipboard receive on   # what a paired phone copies is put on this clipboard
```

or on [Control](control.md)'s **Desktop** page. Only text goes, and only while the screen is not locked.
Anything copied this way also appears in the [clipboard history](clipboard.md#the-clipboard) like any
other copy.

## A code from your phone

A one-time code in a text can show in the [live pill](pill.md) as **Copy 482913** for 60 seconds. To do that the phone
link looks at each text your paired phone receives, on this computer, which is why it is **off** until you say so:

```bash
ame desktop phone codes on
```

or **A code from your phone** on [Control](control.md)'s **Desktop** page, which asks in a terminal.

### What it reads

Only the phone link reads the text: the shell is told a code's digits and nothing else (never the text, the sender or
the phone), and the text is let go once looked at. The code is held for that minute only, in memory; nothing is written
or sent. A code is taken only beside a word that says it is one (code, PIN, OTP and the like, in English, French,
German, Spanish and Arabic), as four to eight digits standing alone, never a phone number, a date, an amount or an
order's number. Other languages, and texts that do not say so, are missed on purpose: a wrong code is worse than none.

### When you copy it

When you copy it, the code is kept out of Amethystora's [clipboard history](clipboard.md#the-clipboard) and is not sent to your
phones, and it comes off the clipboard a minute later unless you have copied something else. Any other program that reads the
clipboard can still read it in that minute, as it can read anything you copy. It is hidden while the screen is recorded or
shared, and while it is locked. Without permission for SMS on the phone, no text ever arrives to look at.
