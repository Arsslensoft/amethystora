# Docks and displays

Docks and screens mostly work as they are plugged in. DisplayLink docks need the image's key enrolled, and external
screens' brightness is a switch you turn on.

## DisplayLink docks

They work with the driver built into the image, whose manager starts when the dock is plugged in and stops when it is
unplugged. A dock whose video comes over USB-C's DisplayPort has no DisplayLink device in `lsusb` (vendor `17e9`) and
needs none of this.

![Plug in a dock](media/dock-screens.webm)

### When the dock stays dark

With Secure Boot on, the dock stays dark until the image's key is enrolled ([Secure Boot](hardware.md#secure-boot)).
When the driver cannot load, a notification says so as the dock arrives, once for each state the machine is in, and
`ame system drivers` keeps saying it:

- **Secure Boot on:** it offers to enroll the key, which asks first and takes a restart through the blue screen.
- **Secure Boot off:** enrolling cannot help, since kernel lockdown refuses the driver, so it offers
  `ame security lockdown off`. Turning Secure Boot on and enrolling is the choice that keeps lockdown.

## Thunderbolt docks and devices

They read and write memory by themselves unless the IOMMU keeps each to what it was given. The
[security report](security.md#see-where-you-stand)'s **DMA protection** check says whether this machine's does, and
`ame security kernel-args iommu.strict=1 on` turns it on where the firmware has VT-d or AMD-Vi.

## External screens' brightness

An external screen's brightness is usually the buttons under its edge. With **Brightness of external screens** on, each
screen that speaks DDC/CI (most made in the last ten years, unless it is turned off in the screen's own menu) gets a
slider in quick settings, after the brightness slider, named as the Displays settings name it. On a laptop the
brightness keys move them with the built-in screen. With it on, `ddcutil` can set them from the command line too.

### Turning it on

It is off, and turning it on asks for an administrator's password: `ame desktop screen-brightness on` (also in Control,
under System) gives the person at the machine the i2c buses of the graphics card's display connectors, which `ddcutil`
talks to the screens on. Such a bus can also reach other chips of the card, so it is a grant the machine's owner makes,
and the [security report](security.md#see-where-you-stand) lists it while it is on. `ame desktop screen-brightness off`
takes it back. A screen's brightness is read when quick settings opens and set when a slider moves; nothing runs
otherwise.
