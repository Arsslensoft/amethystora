# The phone link

The phone link ties an Android phone to this computer: its notifications show here, you read and write its texts in
a window, files and links go both ways, pictures land in Pictures, and the top bar shows its battery and signal. The
phone needs only the **KDE Connect** app (Play Store or F-Droid). Off until you turn it on, and it reaches your phone
only on [networks you named home](security.md#on-from-the-start).

## Turn it on

```bash
ame desktop phone on     # or off: it stops, and does not start at the next login
```

Or the **Phone pairing** switch on [Control](control.md)'s **Desktop** page. It listens for phones only while it is on.
On the phone, install KDE Connect and give it the permissions you want: notifications, texts, contacts (for names in
your texts) and the phone state (for calls).

The phone has to be on a network you named home (`ame security home-networks`, see
[Security](security.md#on-from-the-start)): the firewall keeps it out everywhere else, and this computer announces
itself only there. The link is Amethystora's own program, in place of GSConnect.

## Pair a phone

Open **Phone** in the app grid. Its **Phones** page lists every phone the link knows: paired, away, and nearby but not
paired yet. Pairing shows the same key on both screens; accept only if they match.

![Pairing a phone](media/phone-pair.webm)

1. Open KDE Connect on the phone, where this computer is listed, and ask to pair from there; or press **Pair** on a
   nearby phone on the **Phones** page.
2. A **verification key** of eight characters shows on this computer (in a notification too) and on the phone.
   **Compare them.** If they are the same, the phone in your hand is the one asking: accept on both sides.
3. The request lapses after thirty seconds, with a count beside the key. Nothing here answers for you, and a phone
   you do not recognise has **Reject**.

### Unpairing and looking again

**Unpair** on a phone's card, after a question, ends it from this side and tells the phone. A phone that unpairs from
its side leaves the list and has to be paired again, with the key.

**Look again**, at the top of the page, announces this computer on your home networks once more and tries the last
address of every paired phone: use it when a phone that should be there is not.

## The top bar

While a paired phone is connected, the top bar shows its [battery and signal](#battery-and-signal), or a phone icon
when it has said neither. Its menu has **Send files…**, **Send a link**, **Ping**, **Ring the phone**, **Messages**
(when the phone shares its texts) and **Phones…**, which opens the app.

## Battery and signal

With one phone connected, a small capsule after the [privacy dots](notifications.md#privacy-dots) shows its battery
(a bolt while it charges) and four signal bars, the faint ones above the signal there is. A screen reader reads, say,
"Phone: battery 54 per cent, signal 3 of 4 bars". With two or more phones, the bar shows the phone icon, and each
phone's battery, signal and network ("LTE", "5G") are in its part of the menu.

The signal shows only when the phone tells it: KDE Connect needs permission to read the phone's state, and a phone
without a SIM has none to tell. A phone that goes away takes its battery and signal with it until it connects again.

## What it does not do

The link is deliberately small. A phone cannot run commands here, type on this computer (the
[slide clicker](phone-calls.md#the-phone-as-a-slide-clicker)'s three keys aside, while presenting), control its volume
or mount its storage, and this computer's notifications are not sent to the phone. Players are shared only while
[music from either side](phone-calls.md#music-from-either-side) is on. This computer does not advertise the rest, so
KDE Connect on the phone shows them as unavailable.

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
