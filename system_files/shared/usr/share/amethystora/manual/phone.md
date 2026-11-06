# The phone link

The phone link ties an Android phone to this computer: the phone's notifications show up here, you
read and write its texts in a window, files and links go both ways, pictures land in Pictures, and the top
bar shows its battery and signal. It speaks the protocol of the **KDE Connect** app, so that app, from the Play Store
or F-Droid, is all the phone needs. The link is Amethystora's own program, and it takes the place of
GSConnect. It is off until you turn it on, and it reaches your phone only on [networks you named
home](security.md#on-from-the-start).

## Turn it on

```bash
ame desktop phone on
```

or the **Phone pairing** switch on [Control](control.md)'s **Desktop** page. `ame desktop phone off`
stops the program and keeps it from starting at the next login: it listens for phones only while the
link is on. Nothing else is needed on this side. On the phone, install KDE Connect and give it the
permissions for what you want: notifications, texts, contacts (for names in your texts), and the phone
state (for calls).

The phone has to be on a network you named home (`ame security home-networks`, see
[Security](security.md#on-from-the-start)): the firewall keeps it out everywhere else, and this
computer only announces itself on those networks.

## Pair a phone

Open **Phone** in the app grid. Its **Phones** page lists every phone the link knows: the ones paired
with this computer, the ones that are away, and the ones nearby that are not paired yet.

1. Open KDE Connect on the phone. This computer is in its list. Ask to pair from there, or press
   **Pair** on a phone nearby, on the **Phones** page.
2. A **verification key** of eight characters shows on this computer and on the phone, and a
   notification here carries it too. **Compare them.** If they are the same, the phone in your hand
   is the one that is asking. Accept on both sides.
3. The request lapses after thirty seconds, with a count shown beside the key. A pairing is never
   accepted without you: nothing here answers for you, and a phone you do not recognise has **Reject**.

**Unpair** on a phone's card, after a question, ends it from this side and tells the phone. A phone
that unpairs from its side disappears from the list, and has to be paired again, with the key.

**Look again**, at the top of the page, announces this computer on your home networks once more and
tries the last address of every paired phone. Use it when a phone that should be there is not.

## Notifications

What the phone shows as notifications appears here as ordinary desktop notifications, while it is connected.
Those it already had when it connected are not shown again, and neither are ones it marks as ongoing, such as
a music player's or a download's progress. What a notification offers depends on the phone's app:

- **Reply**, for a message you can answer from the phone's notification: it opens a small window with the
  text and a box to answer in.
- The buttons the phone's own notification has, such as **Archive** or **Mark as read**, which do that on the
  phone.
- **Dismiss**, for one the phone lets you clear. Closing a notification here dismisses it on the phone, and
  dismissing it on the phone takes it away here.

Whatever the phone says is shown as text, never as a command: a link in a notification is not opened by
itself.

## Files and links

**Send:** select a file in Files, then **Open With → Send to Phone** (it is offered for pictures, videos,
sounds, PDFs, text files and archives, and is never what a file opens with by itself); or press **Send…** on a
phone's card, which takes any file; or the phone icon in the top bar. A window asks which phone, if there is more
than one connected, and
shows each file's progress. Folders cannot be sent, only the files in them. A link is sent from the
same window, on its **A link** tab, or from the top bar with the link on the clipboard. The phone
opens it with whatever handles it.

From a terminal:

```bash
amethystora-phone send --device ID report.pdf photo.jpg
amethystora-phone send https://example.org/menu
```

**Receive:** a picture from the phone goes to [Pictures](#photos), and any other file to `~/Downloads/Phone/` in a
folder named for the phone. Neither ever overwrites one that is there: the second `report.pdf` is `report (1).pdf`. A notification offers
**Show in folder**, and **Open** for a picture, a text, a PDF, a sound or a video: any other kind of file is
not opened from here. Nothing opens by itself. Text or a link the phone shares comes as a notification, too,
whose **Show** opens a window with the words as text: select and copy them, and click a web address to open it
in the browser. A web link the phone shares waits in the [live pill](desktop.md#the-live-pill) until you open or
copy it; **Not now** leaves it in the notification list, with **Open** and **Copy**.

## Send to nearby

Select files in Files and choose **Scripts → Send to nearby**, or drop them on Phone in the dock: the send window shows the
phones and computers that can take them now, paired and on a network you named home, as round avatars. Pick one and press
**Send**. Nothing is sent to a device that is not paired, and nothing reaches one that is away.

On the other side, a file from **another computer** waits: a card says who sends what, with **Accept** and **Decline**, and
nothing arrives by itself. Accept once for a folder's worth of files: it covers the rest of what the sender announced, for ten
minutes. Declining, closing the card or leaving it a minute tells the sender to stand down. A phone you paired sends as before,
because it is the one in your hand. To change whose files wait:

```bash
ame desktop phone ask-first computers   # the default: another computer's files wait
ame desktop phone ask-first all         # phones' too
ame desktop phone ask-first never       # every file arrives at once
```

or **Files wait for Accept from** on [Control](control.md)'s **Desktop** page. A file never overwrites one that is there, and lands in
`~/Downloads/Phone/`, as [above](#files-and-links).

## Photos

A picture the phone shares with this computer, from its gallery or its camera, goes to `Phone` in your Pictures folder
(`~/Pictures/Phone`), next to the others the phone sent; any phone's pictures go there, with no folder for each phone. A
picture is a file whose name ends in `.jpg`, `.jpeg`, `.png`, `.heic`, `.heif`, `.webp`, `.gif`, `.avif` or `.dng`; a
video goes with the other files. The name is the phone's, made safe, and a second `holiday.jpg` is `holiday (1).jpg`.
The notification has **Open** and **Show in folder**, and nothing opens by itself. Like any picture you copy into
Pictures, it is read by GNOME's search, which runs its picture reader in a sandbox of its own, so that it can be found
by name in Files and in the search of the overview; pictures kept in Downloads (below) are not.

To keep pictures with the phone's other files in Downloads instead:

```bash
ame desktop phone photos downloads   # or pictures, to put them back in Pictures/Phone
```

or **Photos from the phone go to** on [Control](control.md)'s **Desktop** page.

To add a photo, take it on the phone and share it to this computer from the phone's share menu; it arrives like any picture.
There is no button for this in the KDE Connect app any more, so this computer cannot make a current phone open its camera: the
phone link says only what it has established. See [Snap with your phone](#snap-with-your-phone) for the one case where there is.

## Snap with your phone

KDE Connect once had a request that opened the phone's camera and sent the picture back, and removed it from the phone's app in
2023. A phone that still lists that request (an old install of the app, or a fork) can be asked, and only such a phone. It is
**off**:

```bash
ame desktop phone photo on    # or off
```

or **Snap with your phone** on [Control](control.md)'s **Desktop** page. On, a phone that takes the request has a **Snap with your
phone** button on its card in the Phone app; every other phone has nothing, and the page says to take the picture on the phone and
share it here. Pressing the button asks the phone to open its camera (this computer cannot know that it did, and says only that it
asked), and a notification waits for the picture with **Cancel**, which only forgets the request here: it cannot close the camera
on the phone. One request waits for each phone, for two minutes; a picture that arrives when none was asked for is refused, and one
over 64 MiB too. The picture lands where the phone's other pictures go ([Photos](#photos)), never over a file, and nothing opens by
itself. Nothing but that button can make a phone's camera open: the agent and other programs of your account cannot.

## Texts

**Messages** shows the conversations of a phone that shares its texts, and a chat for each one. Write in
the box below and press `Enter`, or `Shift+Enter` for a new line. **+** begins a conversation, with a
number, or with someone you have written to. **Load older messages** asks the phone for earlier ones.

The words of your conversations are never written to disk: the window shows what the phone has sent the link since it
connected, and a conversation comes and goes with the connection. A message you send waits as *Sending…*
until the phone shows it. There is no delivery report in the protocol: *sent* means the phone has
it, and a message the phone never shows says that it was not confirmed. A group conversation can be
read here and is answered on the phone, and a picture message shows as *Attachment*. Only the names and numbers
of the phone's contacts are kept, in a file only you can read, so that a conversation has a name; unpairing
deletes them.

## Calls

While the phone rings, a notification says who is calling, with **Mute**, which silences the ringer on
the phone. The [live pill](desktop.md#the-live-pill) shows who is calling too, with **Mute ringer**. A missed
call arrives as a notification of the phone's. The link does not change the sound on
this computer, and makes no calls.

## The clipboard

The clipboard is shared in two directions, each its own switch, and both are **off**, since a phone
that can paste into this computer's clipboard can put text where you will paste it into a password
field:

```bash
ame desktop phone clipboard send on      # what you copy here goes to the phones you paired
ame desktop phone clipboard receive on   # what a paired phone copies is put on this clipboard
```

or on [Control](control.md)'s **Desktop** page. Only text goes, and only while the screen is not locked.
Anything copied this way also appears in the [clipboard history](desktop.md#the-clipboard) like any
other copy.

## The top bar

While a paired phone is connected, it appears in the top bar: its [battery and signal](#battery-and-signal), or a phone
icon when it has said neither. Its menu has **Send files…**, **Send a link**, **Ping**, **Ring the phone**,
**Messages** (when the phone shares its texts) and **Phones…**, which opens this app.

## Ringing the phone

Lost the phone somewhere in the house? **Ring the phone** in its top bar menu, **Ring** on its card in the Phone app,
or type *ring* in the search box (`Super+Space`) and press Enter: the phone rings at full volume, even when it is
silent, until **Found it** is pressed on it. Pressing it again stops the ringing. It is one request to a phone you
paired, on a network you named home, as everything else here; in a terminal it is `ame desktop phone ring`.

## Music from either side

With this on, what plays on this computer shows on your paired phone, in KDE Connect's media control, and can be
played, paused and skipped from there; and what plays on the phone shows in the [live pill](desktop.md#the-live-pill)
beside this computer's own music, with its title, its player and the phone's name, played and paused from here. It is
**off**:

```bash
ame desktop phone music on
```

or **Music from either side** on [Control](control.md)'s **Desktop** page. The phone reconnects once, within seconds,
to see the change. Titles, artists and albums go only to the phones you paired, on the networks you named home; no
cover art goes either way, and nothing of this computer's files. While it is off, or while no phone is connected,
nothing watches the players.

## The phone as a slide clicker

While [Present mode](desktop.md#presenting) presents, a paired phone's presenter (KDE Connect's *Presentation
remote*) can go to the next slide, back one, or blank the screen: Page Down, Page Up and B, pressed into the window
in front, which is what slide shows and PDF readers take. Nothing else the phone sends becomes a key, and when
presenting stops, or the screen locks, the phone can send no keys at all: the phone's remote touchpad and keyboard stay
unavailable for this computer. It is **off**:

```bash
ame desktop phone clicker on
```

or **The phone as a slide clicker** on [Control](control.md)'s **Desktop** page. The presenter's pointer does nothing
here.

## A code from your phone

A one-time code in a text can show in the [live pill](desktop.md#the-live-pill) as **Copy 482913** for 60 seconds. To do
that the phone link looks at each text your paired phone receives, as it arrives, on this computer, which is why it is
**off** and you have to say so:

```bash
ame desktop phone codes on
```

or **A code from your phone** on [Control](control.md)'s **Desktop** page, which asks in a terminal. Only the phone
link reads the text: the desktop's shell is told the digits of a code and nothing else, never the text, who sent it or which
phone, and the text is let go once it has been looked at. The code is held for that minute only, in memory. Nothing is
written or sent. A code is taken only beside a word that says it is one (code, PIN, OTP and the like, in English, French,
German, Spanish and Arabic) as four to eight digits standing alone, never a phone number, a date, an amount or an order's
number. A text in another language, or one that does not say so, is missed on purpose: a wrong code is worse than none.

When you copy it, the code is kept out of Amethystora's [clipboard history](desktop.md#the-clipboard) and is not sent to your
phones, and it comes off the clipboard a minute later unless you have copied something else. Any other program that reads the
clipboard can still read it in that minute, as it can read anything you copy. It is hidden while the screen is recorded or
shared, and while it is locked. Without permission for SMS on the phone, no text ever arrives to look at.

## Battery and signal

With one phone connected, the top bar shows its battery and its mobile signal in a small capsule, after the
[privacy dots](desktop.md#privacy-dots): the battery's outline and its charge, a bolt while it charges, and four
bars, the faint ones above the signal there is. A screen reader reads it as, say, "Phone: battery 54 per cent, signal 3
of 4 bars". With two or more phones connected, the bar shows the phone icon, and each phone's battery, signal and
network ("LTE", "5G") are in its part of the menu.

The signal shows only when the phone tells it: the KDE Connect app needs permission to read the phone's state for its
connectivity report, and a phone without a SIM has none to tell. A phone that goes away takes its battery and signal
with it; they come back when it connects and says them again.

## Computers

The same link pairs two computers: your laptop with your desktop, say, to send files and share the clipboard. It is
**off**, which keeps other computers' phone link from showing up here and asking to pair. A device says itself what
kind it is, so this is a tidy list rather than a lock: pairing anything still needs you, and the key on both screens.
Turn it on on **both** computers, each with the phone link on:

```bash
ame desktop phone computers on
```

or **Pair with other computers** on [Control](control.md)'s **Desktop** page. The Phone app's page is then called
**Phones and computers** and lists the other computer. Pairing works as with a phone: both screens show the same key,
and you accept only if they match. Once paired, **Send…** sends files and links (dropping files on Phone in the dock
too), **Ping** shows a notification on it, and the [clipboard](#the-clipboard)'s two switches, still off until you choose, share what you
copy. Files land where a phone's do. As with phones, it works only on networks you named home, and another computer
gets nothing you did not send it: no notifications and no texts.

`ame desktop phone computers off` lets the other computers go: they leave the list, and a computer you paired is kept,
connecting again once this is on. Unpair removes it for good; to unpair a computer you no longer want, turn this on
first, since a computer let go is not in the list. A computer you had paired before this switch existed (a KDE desktop,
say) keeps it on for you. Folders kept the same on both computers are not part of this.

## What it does not do

The link is deliberately small. It does not let a phone run commands here, type on this computer
(the [slide clicker](#the-phone-as-a-slide-clicker)'s three keys aside, while presenting), control its volume, or
mount its storage, and it does not send this computer's notifications to the phone. Its players only while
[music from either side](#music-from-either-side) is on. KDE Connect has features for some of those; this computer does not
advertise them, so the app on the phone shows them as unavailable.

## When something does not work

- **The phone does not show up.** Is the phone on a network you named home, and the link on
  (`ame desktop phone on`)? Press **Look again**. Some routers keep devices apart from each other
  ("client isolation") and nothing can find the phone then.
- **The key differs.** Reject it on both sides: a different key means the other side is not the one
  you chose.
- **A paired phone is not connecting.** Open KDE Connect on the phone once, and check that it is
  allowed to run in the background.
- **Messages stays empty.** The phone's KDE Connect needs the permission to read and send texts, and
  one to read contacts for names. The page says so when the phone does not share its texts at all.
- **No notifications.** On the phone, KDE Connect needs notification access, and each app you want
  mirrored has to be ticked in it.
- **After an update** from GSConnect: a phone paired with it has to be paired again, which a notification
  says once.

## In a terminal

| Command | What it does |
| --- | --- |
| `ame desktop phone [on\|off]` | Turns the link on or off |
| `ame desktop phone clipboard send\|receive [on\|off]` | Shares the clipboard one way or the other |
| `ame desktop phone photos [pictures\|downloads]` | Where pictures from the phone go |
| `ame desktop phone ring` | Rings the paired phones, even when they are silent |
| `ame desktop phone music [on\|off]` | Music from either side |
| `ame desktop phone clicker [on\|off]` | The phone as a slide clicker, while presenting |
| `amethystora-phone` | Opens the app on the phones |
| `amethystora-phone messages [ID]` | Opens it on a phone's texts |
| `amethystora-phone send [--device ID] FILE_OR_LINK...` | Sends files, or one link |
| `amethystora-phone pair ID` | Shows the pairing of a phone, and asks for it if the phone is nearby |
