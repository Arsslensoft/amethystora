# Texts, calls and more

What a paired [phone](phone.md) can do with this computer beyond sharing: texts, calls, ringing it, music, a slide
clicker, and pairing with other computers.

## Texts

**Messages** shows the conversations of a phone that shares its texts, a chat for each. Write in the box and press
`Enter` (`Shift+Enter` for a new line). **+** begins a conversation with a number or someone you have written to;
**Load older messages** asks the phone for earlier ones.

### What is kept

The words of your conversations are never written to disk: the window shows what the phone has sent since it
connected, and a conversation comes and goes with the connection. Only the names and numbers of the phone's contacts
are kept, in a file only you can read, so a conversation has a name; unpairing deletes them.

A message you send waits as *Sending…* until the phone shows it. The protocol has no delivery report: *sent* means the
phone has it, and one the phone never shows says it was not confirmed. A group conversation is read here and answered
on the phone; a picture message shows as *Attachment*.

## Calls

While the phone rings, a notification says who is calling, with **Mute**, which silences the ringer on
the phone. The [live pill](pill.md) shows who is calling too, with **Mute ringer**. A missed
call arrives as a notification of the phone's. The link does not change the sound on
this computer, and makes no calls.

## Ringing the phone

Lost the phone somewhere in the house? **Ring the phone** in its top bar menu, **Ring** on its card in the Phone app,
or type *ring* in the search box (`Super+Space`) and press Enter: the phone rings at full volume, even when it is
silent, until **Found it** is pressed on it. Pressing it again stops the ringing. It is one request to a phone you
paired, on a network you named home, as everything else here; in a terminal it is `ame desktop phone ring`.

## Music from either side

With this on, what plays here shows in KDE Connect's media control on your phone, to play, pause and skip from there;
and what plays on the phone shows in the [live pill](pill.md) beside this computer's music, with its title, player and
the phone's name, to play and pause from here. It is **off**:

```bash
ame desktop phone music on
```

or **Music from either side** on [Control](control.md)'s **Desktop** page. The phone reconnects once, within seconds, to
see the change. Titles, artists and albums go only to paired phones on networks you named home; no cover art goes
either way, nor anything of this computer's files. While it is off, or no phone is connected, nothing watches the
players.

## The phone as a slide clicker

While [Present mode](focus.md#presenting) presents, a paired phone's presenter (KDE Connect's *Presentation remote*)
goes to the next slide, back one, or blanks the screen.

![The phone as a slide clicker](media/phone-clicker.webm)

It is **off**:

```bash
ame desktop phone clicker on
```

or **The phone as a slide clicker** on [Control](control.md)'s **Desktop** page. The phone's buttons become Page Down,
Page Up and B, pressed into the window in front, which slide shows and PDF readers take. Nothing else the phone sends
becomes a key, and the presenter's pointer does nothing. When presenting stops, or the screen locks, the phone can send
no keys at all; its remote touchpad and keyboard stay unavailable here.

## Computers

The same link pairs two computers (your laptop with your desktop, say) to send files and share the clipboard. It is
**off**, which keeps other computers from showing up here and asking to pair. A device says itself what kind it is, so
this is a tidy list rather than a lock: pairing still needs you, and the key on both screens. Turn it on on **both**
computers, each with the phone link on:

```bash
ame desktop phone computers on
```

or **Pair with other computers** on [Control](control.md)'s **Desktop** page. The Phone app's page becomes **Phones and
computers** and lists the other computer. Pair as with a phone: the same key on both screens, accepted only if they
match.

Once paired, **Send…** sends files and links (or drop files on Phone in the dock), **Ping** shows a notification on
it, and the [clipboard](phone-sharing.md#the-clipboard)'s two switches, still off until you choose, share what you
copy. Files land where a phone's do. It works only on networks you named home, and another computer gets nothing you
did not send it: no notifications and no texts. Folders kept the same on both computers are not part of this.

### Letting computers go

`ame desktop phone computers off` takes the other computers off the list; one you paired is kept and connects again
once this is on. Unpair removes it for good: to unpair one you no longer want, turn this on first, since a computer
let go is not in the list. A computer you paired before this switch existed (a KDE desktop, say) keeps it on for you.
