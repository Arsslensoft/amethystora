# Calls and other screens

Checking yourself before a call, a better picture in every app, and letting another computer extend or see this
screen over your home network.

## How do I look and sound?

A meeting's card in the pill offers **How do I look and sound?**: a small window with your camera, a microphone level
and the name of the microphone in use. It says so when nothing is heard, or when a Bluetooth headset's microphone would
lower your sound.

- Close it when you have seen enough. It closes by itself after three minutes, when the screen locks, and when the
  screen starts being shared, and it is not offered while it is shared.
- The camera opens only while the window shows, so its light is on only then, and the privacy dots say so. Nothing is
  recorded or sent.
- A camera blocked in Security stays blocked, and the window says it found none.

`ame desktop pill mirror off` takes it out of the card.

## Another screen from what you have

`ame desktop second-screen on` turns an old laptop or tablet into a **second screen** next to this one, over your home
network, with GNOME's own remote desktop. It shows a **new twelve-character code**, the address to type into any RDP app
on the other computer and the certificate's fingerprint, and a virtual monitor then appears next to yours. The other
computer has never seen the certificate and says so: compare the fingerprint it shows with the one printed here before
you accept it, and only then type the code.

### Before you start

- It needs the Security switch **Let another computer see or extend this screen** (`ame security remote-screen on`, off
  as shipped), which opens its one port in the firewall's home zone only. It refuses while that switch is off or the
  firewall is off.
- **The other computer's keyboard and pointer can use this whole computer**, every window on it, not only its own
  screen.
- If you set a username and password in GNOME Settings' Remote Desktop, it says so and asks first: they cannot be kept
  while this runs.

### How long it lasts

It makes a certificate for this alone and starts the service for this session only. The code stays valid until you
turn it off, **lock the screen**, or two hours pass. `ame desktop second-screen off` ends it and puts the settings
back, and the code no longer works. Across the internet it cannot work, because that needs a relay, and Amethystora
runs none: use it at home.

## Help from someone you trust

`ame desktop help-me on` lets a person on your home network **see your whole screen and nothing more**. It needs the
same Security switch (`ame security remote-screen on`, one port in the home zone only), shows a new code each time, and
the other side connects with any RDP app.

- **Close what is private first**: nothing here promises that windows such as Notes or the Vault are hidden from them.
- They cannot move the mouse or type until you allow it with `ame desktop help-me control on`; `control off` takes it
  back.
- It ends by itself when you lock the screen (within a second) or after an hour, and `ame desktop help-me off` ends it
  at once; the code is good until then. GNOME shows its own indicator in the top bar while anyone is connected.
- **Across the internet it cannot be built here:** reaching a computer behind a router needs a relay both sides can
  dial out to, and Amethystora runs no servers. Until there is a way that needs no relay, it works on the home network
  only.

## Look good in every call

`ame desktop camera on` blurs what is behind you and evens out the light in every app at once, instead of a setting in
each. It is off until you choose it. Apps then see a second camera, the virtual one: pick it in each app.

- It downloads one small model once (6 MiB, Apache-2.0, checked against the SHA-256 in the image).
- It measures what 720p video costs on this computer and **turns on only where that is under 15% of one processor
  core**: on an older or busy computer it says so and does not offer it. Turning an effect on measures again.
- It runs only while an app has that camera open (through PipeWire, or itself as browsers do). With no camera it ends
  and does not start again by itself: run `ame desktop camera on` after plugging one in.
- Your video is made here and never written, logged or sent. Blocking the webcam in [Security](security.md) still
  blocks it, since then there is no camera to read.

```bash
ame desktop camera blur on     # or off
ame desktop camera light on    # or off
ame desktop camera off
ame desktop camera remove      # deletes the model
```
