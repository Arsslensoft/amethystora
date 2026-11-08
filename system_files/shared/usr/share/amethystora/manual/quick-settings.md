# Quick settings

The menu at the right end of the top bar opens on tiles, two to a row, each saying what it knows: the Wi-Fi network
and its strength, the earbuds and their battery, the power mode and, while a paired phone is connected, the phone and
its battery (click it for the Phone app). Click a tile to switch it, and its arrow for its menu. Under the tiles are the
sliders, the apps playing sound, a swatch for every theme, and Screenshot, Lock, Settings and Power.

![Quick settings](media/quick-settings-tiles.webm)

## Moving and hiding tiles

Hold a tile for half a second (or press `Menu` on it): the tiles shrink a little and each gets a round button at its
corner. Drag a tile to its new place; the button puts it away into a Hidden row, where the same button brings it back.
Done, `Esc` or closing the menu ends it.

- Keyboard: `Ctrl` and an arrow moves the tile you are on, `Delete` hides it, `Enter` brings a hidden one back.
- `ame desktop quick-settings reset`, or **Quick settings tiles** in [Control](control.md), puts every tile back in
  its first place and shows them all.

## Keep the battery at 80%

On a laptop the first tile is the battery: how full it is, and when it runs out or is full. Its menu has **Keep the
battery at 80%**, so the laptop stops charging early and the battery lasts more years. It is GNOME's own charge limit,
which UPower keeps across restarts; a laptop that cannot stop charging early has no such switch.

## Each app's volume

While apps play sound, each has its own slider and mute button under the volume slider, and the buttons beside
"Playing now" send its sound to another output. To find what is making a noise, look at the top: apps sending sound
come first, a sandboxed app goes by its own name, and an app that only holds its sound open says **Paused**, dimmed.

It asks the sound server what each stream is while the menu is open and never listens to one, so it cannot tell which
is loudest, and an app sending silence counts as sending. `ame desktop quick-settings sounding off`, or **Which app is
making sound** in Control, lists them all by name instead.

## Theme swatches

A tap on a swatch switches to that theme, as `ame desktop theme` does: the desktop and Amethystora's apps change at
once, other apps the next time they start. `ame desktop quick-settings theme off` hides the swatches (`on` brings them
back), or hide them as you hide a tile.

## Send your screen to a TV

`ame desktop send-to-tv on`, or **Send to the TV** in [Control](control.md), adds a tile. Pressed the first time, its
menu says what it needs: GNOME Network Displays, a free app from Flathub, which a terminal fetches after you press
**Install** and say yes (`ame desktop send-to-tv setup`). From then on the tile opens that app, which lists the TVs and
screens it finds; you pick one there, then what to show. Nothing runs until you press the tile.

**Nothing here has been seen working with a real TV yet.** It is written so that a screen cast is a screen share like
any other: the pill should say so while it lasts, the glow should go round the screen, and the
[share guard](screen-privacy.md#share-guard) covers the windows on its list while the whole screen is shared. Try your
TV before the meeting.

- **Casting over your network**, as from a phone, should find a TV or stick if the network lets devices find each
  other. With `ame security firewall-away on`, a network you have not named home answers nothing, discovery included;
  with `ame security network-daemons on`, Avahi (which finds devices) is stopped, so nothing is found.
- **Miracast** (a TV reached straight over Wi-Fi Direct) needs a firewall zone of Network Displays' own, which its
  Flatpak cannot bring and Amethystora does not carry or open: it is not expected to work.
- **What the app may do.** It may talk to NetworkManager, Avahi and the firewall on the system bus. It can read the
  firewall's configuration and may ask for a password to change it: refuse a firewall prompt you did not expect, since
  the firewall is set in [Security](security.md), not by this app.

## Tools at hand

### Pick a colour

`Super+Alt+C` holds the screen still and puts a loupe under the pointer, with the colour under it named. Click copies it
as `#9148F0`, `Shift`+click as `rgb(145, 72, 240)`, `Ctrl`+click as `hsl(266, 85%, 61%)`; `Esc`, a right click or the
key again closes it with nothing copied. The colour goes on the clipboard (and its history), and your last eight are in
the search: type `colour:`. Nothing of the screen is kept once the loupe closes, and it does not open while the screen
is shared.

### Mute the microphone

`Super+Alt+M` mutes the microphone for every app at once, in a call or not: the input's own mute, the one the
microphone slider sets. While muted, **Mic off** stays in the top bar; its menu, the key again or the slider turns it
back on. A microphone you plug in meanwhile (a headset) is muted too. The
[privacy dots](notifications.md#privacy-dots) list has the same **Mute** beside the apps using the microphone.

### Share this Wi-Fi

The Wi-Fi menu has **Share this Wi-Fi** while you are on a network a phone can join with its password alone. It shows a
QR code that a guest's phone camera joins with, no password read aloud or typed. The password is read only then, may
need yours to read, and goes when the menu closes. Not offered for a network that needs a user name (802.1X) or for
this computer's own hotspot, and not shown while the screen is shared or recorded.

### Is it me or the internet?

The Wi-Fi and wired menus draw you, the router and the internet, with the line that broke in red, and say in a sentence
what it means and what to try. A network that wants you to sign in shows amber.

Each time the menu opens, one ping goes to your router and, while the [network connectivity check](what-leaves.md) is
on, one lookup of the name that check already asks goes to the name server this machine already uses (encrypted if you
chose [encrypted DNS](network-security.md#encrypted-dns)). With that check off, no name is looked up and the drawing
says less. That is all it contacts, and nothing runs while the menu is closed.

A router may simply not answer pings, so a silent router is not blamed by itself, and a name server can fail alone
while the internet answers: the sentences say "may". For a connection that is not the one your internet goes through,
only the router is asked. It is listed in `ame security sends`. `ame desktop network-check on|off`, also in
[Control](control.md).
