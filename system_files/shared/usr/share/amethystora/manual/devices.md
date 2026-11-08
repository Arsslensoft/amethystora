# Devices

Most printers, keyboards, controllers, sound cards and tablets work as they are plugged in. This page covers the few
that need a step from you.

## Printers and scanners

Most printers are found on the network and just work, without a driver. Drivers for HP printers and many Brother and
older laser printers are included for the rest. Add a printer in **Settings → Printers**.

## Keyboards, mice and security keys

- **Gaming mice:** the service that configures them is included; install **Piper** from Bazaar to change buttons,
  lighting and resolution.
- **Remapping keys:** `ame security input-remapper` turns on Input Remapper
  ([Security](devices-security.md#key-remapping) explains why it starts off).
- **FIDO2 security keys** can replace your password: [Security](logins.md#security-keys).

### Keyboard configurators

VIA, Vial, Keychron Launcher, ZSA's Oryx and Keymapp, and Wootility reach Keychron, Lemokey, ZSA, Vial and Wooting
keyboards for whoever is at the machine. Other keyboards are kept from them and from every other program, because what a
configurator reads is every key typed. To configure another one, give it to whoever is at the machine with its vendor ID
from `lsusb`, then plug it in again:

```bash
echo 'KERNEL=="hidraw*", ATTRS{idVendor}=="1234", TAG+="uaccess"' | sudo tee /etc/udev/rules.d/70-keyboard.rules
```

## Game controllers

Xbox, PlayStation, Nintendo and Steam controllers work over a USB cable or Bluetooth as soon as they are connected. Xbox
controllers over Bluetooth get rumble, their battery level and the right button mappings from xpadneo, which is in the
image.

### Microsoft's wireless adapter

The adapter for Xbox controllers, and the one some laptops have built in, needs Microsoft's firmware, which the image
may not pass on. This downloads it from Microsoft, checks it against the checksum Amethystora pins, and points the
kernel at it from the next restart:

```bash
ame apps xbox-adapter on
```

- The firmware is subject to Microsoft's terms of use.
- The adapter is a MediaTek MT7612U chip, so Linux's Wi-Fi driver for that chip is kept from loading while it is on: a
  USB Wi-Fi stick with the same chip (the Netgear A6210 and many AC1200 sticks) stops working until
  `ame apps xbox-adapter off`.
- A [sealed image](streams.md#sealed-images) cannot take firmware from outside the image, so there the adapter cannot be
  set up; controllers still work over Bluetooth or a cable.

## Sound cards

USB sound cards and audio interfaces work as they are plugged in. FireWire ones need FireWire, which is blocked, since a
FireWire device can read and write the machine's memory by itself: `ame security firewire off` lets it load, for
FireWire sound cards, DV cameras and disks, and `on` blocks it again ([Switches](security-switches.md#switches)). The
Neutron HiFi DAC V1's own settings app reaches it for whoever is at the machine.

## Drawing tablets

Most drawing tablets work as soon as they are plugged in, and GNOME's Settings has a page for one once it is. For a
tablet the kernel's drivers handle badly (often Huion, XP-Pen and Gaomon), or to set every tablet up the same way, use
OpenTabletDriver:

```bash
ame apps opentabletdriver on
```

### What it changes

- It installs OpenTabletDriver from Flathub, copies its device rules into `/etc` from the release Amethystora pins
  (checked against that release's checksum), and starts its driver at each login.
- It turns off the kernel's own tablet drivers, which would take the tablet first, so restart if the tablet was plugged
  in.
- It moves the pointer through `/dev/uinput`, which the image keeps from apps, so it gives that back to whoever sits at
  the machine ([Switches](security-switches.md#switches), `ame security virtual-input`).

Set the pen's pressure, its buttons and the part of the screen the tablet covers in **OpenTabletDriver**, in the app
grid. `ame apps opentabletdriver off` takes it all away, and the kernel's drivers come back after a restart. The
**Drawing tablets** switch on [Control](control.md)'s **Apps** page does either in a terminal.

## Cameras

Virtual cameras (OBS's, for instance) work without anything to install, through the loopback module built into the
image.
