# Devices and your screen

Switches for what may reach this computer through a port or a cable: another computer seeing the screen, USB devices,
and the program that remaps keys.

## Letting another computer see this screen

`ame desktop help-me` (someone you trust sees your screen) and `ame desktop second-screen` (an old laptop or tablet as a
second screen) use GNOME's remote desktop, and **need one port, 3389, open on the networks you named home**. That is
this switch, and it is **off**:

```bash
ame security remote-screen on
```

or **Let another computer see or extend this screen** on Security's **Settings** page, under Network.

- It adds the remote screen to the home zone of the firewall, and nothing else: nothing listens on the port until you
  start one of the two features. Turning it off closes it again.
- The two features refuse to start while the switch is off, and while the firewall itself is off (every network would
  then get the port).
- Anyone on your home network who learns a code, or watches you type it, sees what is on the screen.

## Block USB devices you did not plug in

A USB device can claim to be a keyboard and type by itself, and a hardware keylogger can sit between a real keyboard and
the machine. USB protection has two modes. It is off by default, because a machine that blocked its own keyboard on
first boot would be unusable.

```bash
ame security usb
```

### While the screen is locked

`ame security usb locked`: a USB device plugged in while the screen is locked is blocked, so that nobody can plug one
into a machine left locked. What is plugged in at boot works, and so does what you plug in while the screen is
unlocked.

![A new device waits for the unlock](media/usb-locked-refuse.webm)

While it is locked:

- a plain keyboard or mouse still works, so that you can type your password, and a notification says so;
- anything else stays blocked: a USB stick, a network adapter, a keyboard that is also something else (many gaming
  keyboards are). A notification says to plug it in again once you have unlocked, and then it works. GNOME shows one
  such notification at a time, so with two devices you may see only one;
- a laptop's own built-in devices always work.

What counts as unlocked, and the limits:

- The login screen counts as unlocked, and so does another account's unlocked session on the screen: a device plugged
  in there works. Once the login screen has been shown over your locked session, to switch user, new devices are let in
  at your lock screen too, until you unlock and it locks again.
- GNOME does the blocking. The USB protection setting under **Privacy** in Settings shows it, greyed out, since it is
  switched here for the whole machine. If GNOME's part is not running, new devices are let in as when the screen is
  unlocked.
- A dock whose devices come back as new after sleep may need plugging in again.
- A program running as you in your session could turn this mode off, as it could do anything else you can.

### Every new device

`ame security usb on` allows what is connected when you set it up and blocks anything new, locked or not, until you
allow it with `ame security usb allow`, which keeps it allowed after a restart too. Plug in everything you use first:
keyboard, mouse, dock, webcam, headset and security keys.

`ame security usb off` trusts every USB device again.

## Key remapping

Input Remapper runs as root and reads every key typed into every application. That is what remapping needs, and a
keylogger in every other respect, so it ships switched off. `ame security input-remapper` turns it on;
`ame security input-remapper off` turns it off again and keeps your mappings.
