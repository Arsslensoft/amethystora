# Hardware

Amethystora runs best on hardware that Linux supports well: Intel and AMD graphics, and the laptops
their makers certify for Linux. Most things work the moment they are plugged in; this page is about
the ones that need a step from you.

## Secure Boot

Leave Secure Boot on. With it, the machine only boots a kernel it has checked, and the kernel's
[lockdown](security.md) has something to stand on.

The kernel modules built into the image, for DisplayLink docks, virtual cameras and Xbox controllers
over Bluetooth or Microsoft's wireless adapter, and NVIDIA's driver on the NVIDIA images, are signed with
keys of the image's own, and the firmware has to be told to trust them once:

```bash
ame security secure-boot
```

Restart, and a blue screen appears before the system starts: the MOK manager. Choose
**Enroll MOK**, then **Continue**, then **Yes**, and type the password `amethystora`. The screen uses
a US (QWERTY) keyboard layout whatever your own is, which matters if your keyboard puts the letters
elsewhere. `ame security status` tells you whether the keys are enrolled.

If Secure Boot is off, a notification says so once. Turn it on in the machine's firmware settings
(`ame system bios` restarts into them), then run the command above.

A [sealed image](updates.md#sealed-images) needs Amethystora's key before it can start at all, so its
installer asks the MOK manager for it, together with the modules' key: the blue screen comes at the
first start, and asks the same password. An NVIDIA image installed from its ISO with Secure Boot on
does the same, since NVIDIA's driver does not load without the key.

## Graphics

- **Intel and AMD** work out of the box. The developer image adds ROCm for GPU compute on AMD.
- **NVIDIA** needs one of the NVIDIA images ([Installing](installing.md#pick-an-image)), which carry
  NVIDIA's driver, and which one depends on the card:

  | Card | Image |
  | --- | --- |
  | GeForce 16 and RTX 20 series and newer (Turing on) | `-nvidia-open`, NVIDIA's open driver |
  | GeForce 900 and 10 series, the GTX 750, Titan V (Maxwell, Pascal, Volta) | `-nvidia-lts`, NVIDIA's closed driver of its 580 branch, the last that drives them |
  | Older (Kepler and before) | none: the image without NVIDIA's driver, whose open driver nouveau draws on them |

  The NVIDIA driver comes built for the image's kernel and signed with a key of the image's own, which
  the kernel trusts only with Secure Boot on. So the NVIDIA images turn kernel lockdown on only with
  Secure Boot, as the kernel itself does, never with an argument of their own. With Secure Boot on,
  enroll the keys ([above](#secure-boot)) before switching to an NVIDIA image: `ame system rebase`
  offers to and switches only once they are, since the screen would otherwise stay black.
- **Installed an image whose driver does not drive the card?** A few minutes after the first boot a
  notification offers **Switch images…**, which opens `ame system rebase` on the image that does:
  the `-nvidia-open` or `-nvidia-lts` image on a machine with an NVIDIA card, and on an NVIDIA image
  with no card NVIDIA's driver drives, the image without it, which keeps lockdown on. It takes signed
  images only and asks before it switches. Nothing switches by itself, it is said once for the
  machine, and **Don't ask again** stops it for good.

## Docks and displays

- **DisplayLink docks** work with the driver built into the image, whose manager starts when the dock is
  plugged in and stops when it is unplugged. With Secure Boot on, the dock stays dark until the key above
  is enrolled. When the driver cannot load, a notification says so as the dock arrives, once for each state
  the machine is in, and `ame system drivers` keeps saying it: with Secure Boot on it offers to enroll the
  key, which asks first and takes a restart through the blue screen. With Secure Boot off, enrolling cannot
  help: kernel lockdown refuses the driver, so it offers `ame security lockdown off`, and turning Secure Boot
  on and enrolling is the choice that keeps lockdown. A dock whose video comes over USB-C's DisplayPort has
  no DisplayLink device in `lsusb` (vendor `17e9`), and needs none of this.
- **External monitor brightness**: see [External screens' brightness](#external-screens-brightness). With it on,
  it can also be set from the command line with `ddcutil`, for monitors
  that support it.
- **Thunderbolt docks and devices** read and write memory by themselves unless the IOMMU keeps each to
  what it was given. The [security report](security.md#see-where-you-stand)'s **DMA protection** check
  says whether this machine's does, and `ame security kernel-args iommu.strict=1 on` turns it on where
  the firmware has VT-d or AMD-Vi.

### External screens' brightness

An external screen's brightness is usually the buttons under its edge. With **Brightness of external screens** on,
each screen that speaks DDC/CI (most made in the last ten years, when it is not turned off in the screen's own menu)
gets a slider of its own in quick settings, after the brightness slider, named as the Displays settings name it. On a
laptop the brightness keys move them with the built-in screen.

It is off, and turning it on asks for an administrator's password: `ame desktop screen-brightness on` (also in
Control, under System) gives the person at the machine the i2c buses of the graphics card's display connectors, which
`ddcutil` talks to the screens on. Such a bus can also reach other chips of the card, so it is a grant the machine's
owner makes, and the [security report](security.md#see-where-you-stand) lists it while it is on.
`ame desktop screen-brightness off` takes it back. A screen's brightness is read when quick settings opens and set when
a slider moves; nothing runs otherwise.

## Memory

When RAM runs short, what programs are not using moves into compressed memory (zram) rather than to the
disk, and comes back when a program needs it. It is compressed quickly as it leaves RAM, and what then
stays unused is repacked smaller every hour while the machine is on mains power. On a 4 GB machine
holding 3 GB of program data, that took about 420 MB of RAM for each GB moved out, against 540 MB with
the quick compression alone, and brought it back sooner than compressing it small from the start.

The kernel's own memory settings are left as they are. The values often suggested for zram
(`vm.swappiness` at 180, `vm.page-cluster` at 0, and the two watermark settings) were measured on 4 GB
and 8 GB machines, together and one at a time: none made the desktop quicker, and a swappiness of 180
made it slower. Performance mode in the quick settings swaps less (a swappiness of 10), which did no harm.

`ame system zram` explains the choices and asks, and so does **Swapped-out memory** on
[Control](control.md)'s **System** page: `default`, `fast` (no repacking, Fedora's own) or `compact`
(small from the start, slower to come back). A choice takes effect from the next restart.

## Disks

- **Encryption** is chosen when installing and cannot be added afterwards. Once it is on,
  `ame security disk-unlock` lets the TPM unlock the disk at boot instead of you typing the
  passphrase ([Security](security.md#unlock-the-disk-with-the-tpm)).
- **Several disks** can keep copies of each other: [Pools of disks](#pools-of-disks-raid).
- **ZFS** is available on the `stable` stream.
- **Apple's USB SuperDrive** is woken up as it is plugged in, as a Mac would, so that it reads discs.

### When a disk may be failing

Disks usually say they are failing long before they do. Every hour the disks the system is on are asked for their health
(SMART), and a disk asleep is left asleep. If one says it is failing, or starts counting bad sectors or media errors, a
notice says so once, or again if the count rises, with **Back up now** and how long it has been since your last backup.
A disk that already had some bad sectors when it was first read is not said to be starting to fail: only a rise is news.

It may be wrong either way: a disk that reports errors can run for months, and a disk can die without a word. So take
it as the moment to copy what matters, and [Backups](security.md#backups) is the quickest way. Then replace the disk.
A disk that belongs to [a pool](#pools-of-disks-raid), the system's own included when it is on several disks, is told of
by the pools' check when it says it is failing and when btrfs counts errors on it, with what to do about it; this says only
what that check cannot, so that one disk is not the subject of two notices.

`ame system disk-warning` says what each disk said last. `ame system disk-warning off` stops reading them, and nothing
then warns you; `on` starts again, both also in [Control](control.md). What is kept, in
`/var/lib/amethystora/disk-health.json`, which any account can read: for each disk its model, a checksum of its serial
number (to tell it from another when their names swap), its name as the system has it now, what it said (its health and
its count of errors) and what was told of it.

## Pools of disks (RAID)

Two disks or more can hold the same files, so that one can fail without taking a file with it. That is
a pool. The **Disks** group on [Control](control.md)'s **System** page shows them and makes them, or in
a terminal:

```bash
ame system raid              # how the pools are, and what can be done next
ame system raid create       # a pool from spare disks
```

`create` offers only the disks nothing is using: never the system's own, nor one with anything mounted
or open on it. Pick the disks, a layout and whether to encrypt, and type `ERASE`: everything on those
disks is erased, and the list shows what is on each first. Each question has its answer chosen
already, so it can be Enter, Enter, Enter and `ERASE`. The pool is then at `/var/mnt/data` (`data2`
for a second one), belongs to you, and is in the Files sidebar. `ame system raid create --name media
/dev/sdb /dev/sdc` names the pool and its disks ahead: a name of up to 31 lower-case letters, digits and
dashes, and two spare disks or more.

| Layout | From | Survives | Worth knowing |
| --- | --- | --- | --- |
| **Two copies** | 2 disks | one dead disk | The one to choose unless you have a reason not to |
| **Three copies** | 3 disks | two dead disks | A third of the space for files |
| **Striped two copies** | 4 disks | one dead disk | Faster for large files, and as safe as two copies |
| **Parity** | 3 disks | one dead disk | btrfs's own advice: not for production, only for testing. A power cut can damage what was being written: check every copy after one |
| **Double parity** | 4 disks | two dead disks | The same as parity |
| **Combined** | 2 disks | none | Joins the disks into one and keeps no copies: a dead disk takes its files with it |

The list of layouts says how much each would hold on the disks you picked.

- **Encrypted** unless you say no. The pool's key is kept on the system disk, so the pool opens with
  the system and is as safe as the system disk is, and a pool disk that leaves the house, sent back
  under warranty or thrown away, gives nothing away. `create` shows a **recovery key** once: write it
  down. It opens the pool's disks on any Linux machine, and without it a pool whose system disk is
  lost is lost too.
- **A pool is not a backup.** A file deleted, or encrypted by ransomware, is gone from every disk at
  once. [Backups](security.md#backups) cover your home folder; to back up a pool as well, add its
  folder to `AMETHYSTORA_BACKUP_PATHS` in `~/.config/amethystora/backup.env`, after your home folder.
- **A disk on USB** can be part of a pool, but a loose cable takes it out: keep it plugged in.

### When a disk fails

A pool that loses a disk goes on working, which is why nobody notices until the second one goes. So
Amethystora looks every hour and says so in a notification: which pool, which disk by its model and
serial number, and whether every file is still there. **Open Control** goes to the Disks group, where
the pool says what happened and offers what to do:

- **Replace a disk** (`ame system raid replace NAME`) takes a spare disk at least as large, erases it
  and copies onto it what the old one held. It asks for the pool's recovery key, to put it on the new
  disk too; left empty, every disk of the pool gets a new one. It goes on if the window is closed.
  btrfs needs a gigabyte free on each disk that is left to rebuild onto a new one, so a pool too full
  for that is refused before anything is erased: delete what you can spare, then try again. A replace
  that stops part way leaves the new disk blank again, to be picked once more.
- **Open it with a disk missing** (`ame system raid mount NAME`) is for a pool that did not open
  because a disk was missing when the machine started. What is written to it then has one copy until
  the disk is replaced. A pool missing more disks than its layout survives opens read-only, so that
  what is left can be copied off.
- **Put the copies back** comes up when the missing disk came back. What was written while it was
  away has one copy, and the disk itself still has the old copies of it: btrfs reads the good copy and
  counts the old one as an error, so the disk shows as **back**, not as failing. Putting the copies
  back checks every copy, rewrites the old ones from the good ones, gives what has one copy its second,
  and sets the disk's count back to zero. A file btrfs keeps no checksum of (one marked No_COW, such
  as the journal's) cannot be told apart that way, so its copy on that disk may stay old.
- A disk **counting errors** had reads or writes fail. btrfs read the other copy, so no file was lost,
  but a count that keeps rising means the disk, or its cable, is going. Once the cause is fixed,
  `sudo btrfs device stats --reset /var/mnt/NAME` sets the count back to zero.

### Looking after a pool

- **The monthly check** reads every copy, on mains power, and repairs a damaged one from a good one.
  **Check every copy now** (`ame system raid scrub now`) does it straight away: do that after a power
  cut on a pool with parity. `ame system raid scrub off` turns the monthly check off and `on` back on;
`ame system raid scrub` says whether it is on and offers to change that.
- **Add a disk** (`ame system raid add NAME`, or `add NAME DISK` to name it) makes room for more, on an
  open pool. It asks whether to spread what is
  already there over every disk now, which takes hours on a large pool.
- **Forget** (`ame system raid forget NAME`) stops mounting a pool and leaves its files on its disks.
  Without a `NAME`, each of these takes the only pool, or asks which.
  GNOME Disks' **Format** erases them when you want the disks back.

### The system's own disks

The installer makes no pools: install on one disk, and make a pool of the others afterwards. The system
itself can be on two disks as well, so that the machine goes on with one of them dead:

- **Mirror the system onto a new disk** (`ame system raid mirror DISK`) copies a system installed on
  one disk onto a spare disk at least as large: its own EFI partition, a half of `/boot` (the two
  halves are a RAID1, `md-boot`), and a copy of every file, encrypted with the system's passphrase if
  the system is. It erases that disk once you type `ERASE`, and nothing on the system's disk. Once the
  copy is done, the machine restarts once from the new disk; when that start has worked, the first
  disk's `/boot` joins the new one's, and from then on the machine starts from either disk. The
  passphrase is still typed once, and a [TPM](security.md#unlock-the-disk-with-the-tpm) that unlocked
  the disk gets a key for the new one too, with its PIN, as the command asks.
- **Protect the system's disks** (`ame system raid mirror`) is for a system the installer spread
  over two disks with one copy of each file, where either one failing loses everything. It keeps a
  copy of each file on each disk instead, and erases nothing. The boot files stay on the first disk:
  the machine starts from it, and with that disk dead every file is still on the other, readable from
  the live USB stick.

A sealed machine cannot be mirrored: its signed kernel command line names one disk.

**Starting with a disk dead.** Without one of its disks, the system waits for it at the start, with
no end. Turn the machine off and on again, and in the boot menu, which shows for five seconds on a
system over several disks, choose **Start with a disk missing**: the system starts from the disk that
is left, and says which disk is missing. Then **Replace a disk** (`ame system raid replace system`)
puts a new disk, at least as large, in its place, with an EFI partition and a half of `/boot` of its
own, and **Put the copies back** mends what was written in the meantime once the disk itself comes
back. A machine whose firmware does not start from the second disk by itself needs it chosen once in
its boot menu (often F12 or F8).

## Printers and scanners

Most printers are found on the network and just work, without a driver. Drivers for HP printers and
many Brother and older laser printers are included for the rest. Add a printer in
**Settings → Printers**.

## Phones

- **Android:** the phone link works with the KDE Connect app on the phone: its notifications, files and
  links sent either way, texts and calls, and a clipboard you can share ([Phone](phone.md)). It is off until
  you turn it on, with `ame desktop phone on` or the **Phone pairing** switch on [Control](control.md)'s
  **Desktop** page: the program listens only while it is on, and `ame desktop phone off` stops it.
  Install KDE Connect on the phone and pair them from the **Phone** app, on a network you have named home:
  elsewhere the firewall keeps the phone out ([Security](security.md#on-from-the-start)). Name it with
  `ame security home-networks`. A phone paired with GSConnect, which the phone link replaced, has to be
  paired again, which a notification says once.
- **iPhone:** plug it in and trust the computer on the phone; its photos and files show up in Files.

## Keyboards, mice and security keys

- **Gaming mice:** the service that configures them is included; install **Piper** from Bazaar to
  change buttons, lighting and resolution.
- **Remapping keys:** `ame security input-remapper` turns on Input Remapper ([Security](security.md#key-remapping)
  explains why it starts off).
- **Keyboard configurators** (VIA, Vial, Keychron Launcher, ZSA's Oryx and Keymapp) reach Keychron,
  Lemokey, ZSA, Vial and Wooting keyboards (Wootility) for whoever is at the machine. Other keyboards are kept from them and
  from every other program, because what a configurator reads is every key typed. To configure another
  one, give it to whoever is at the machine with its vendor ID from `lsusb`:
  `echo 'KERNEL=="hidraw*", ATTRS{idVendor}=="1234", TAG+="uaccess"' | sudo tee /etc/udev/rules.d/70-keyboard.rules`,
  then plug it in again.
- **FIDO2 security keys** can replace your password: [Security](security.md#security-keys).

## Game controllers

Xbox, PlayStation, Nintendo and Steam controllers work over a USB cable or Bluetooth as soon as they
are connected. Xbox controllers over Bluetooth get rumble, their battery level and the right button
mappings from xpadneo, which is in the image.

**Microsoft's wireless adapter for Xbox controllers**, and the one some laptops have built in, needs
Microsoft's firmware, which the image may not pass on. This downloads it from Microsoft, checks it
against the checksum Amethystora pins, and points the kernel at it from the next restart:

```bash
ame apps xbox-adapter on
```

The firmware is subject to Microsoft's terms of use. The adapter is a MediaTek MT7612U chip, so
Linux's Wi-Fi driver for that chip is kept from loading while it is on: a USB Wi-Fi stick with the
same chip (the Netgear A6210 and many AC1200 sticks) stops working until `ame apps xbox-adapter off`.
A [sealed image](updates.md#sealed-images) cannot take firmware from outside the image, so there the
adapter cannot be set up; controllers still work over Bluetooth or a cable.

## Wi-Fi

Nearly every Wi-Fi card works without anything to install. A few of Broadcom's (the BCM4360, 4352
and 43142, in older MacBooks and some laptops) have no driver on Amethystora: the only one there is,
Broadcom's own `wl`, has security holes that anyone in Wi-Fi range can reach (CVE-2019-9501 and
CVE-2019-9502) and that its licence forbids anyone but Broadcom to fix, and Broadcom stopped
updating it in 2015. A USB Wi-Fi adapter works instead.

## Sound cards

USB sound cards and audio interfaces work as they are plugged in. FireWire ones need FireWire, which
is blocked, since a FireWire device can read and write the machine's memory by itself: `ame security
firewire off` lets it load, for FireWire sound cards, DV cameras and disks, and `on` blocks it again
([Switches](security.md#switches)). The Neutron HiFi DAC V1's own settings app reaches it for whoever
is at the machine.

## How long it takes to start

Computers are expected to slow with age. This one replaces its system whole with every update, so the system itself
cannot wear out, and it keeps the numbers to show it: one for each start, by date, on **Control › System**, with a graph.

```bash
ame system start-times      # what it measured
ame system start-times why  # when it got slower: what changed
ame system start-times off  # stop measuring; what was measured is kept
```

The number is what it measured, on the days it looks bad as well. It runs from the kernel starting to the system having
started, the login screen with it. Unlocking the disk is left out, since the time you take to type a password is not the
computer being slow, and so are the firmware and the boot menu. A restart that keeps the kernel after an update
([soft reboot](updates.md#restarting-in-seconds)) is not a start.

It says "Still as fast as day one" only for a computer that has been measured since its first days. One that was updated
into this says "Still as fast as when it began to be measured", and since when. If lately it takes a tenth longer than in
the first week and at least a second, it says "a little more slowly", and if it takes a quarter longer and at least three
seconds, "more slowly", here, in `ame system doctor` and under Something's wrong. **What changed** (a button on the card,
or `ame system start-times why`) names the update it got slower with, if it did, and what took longest in this start,
leaving out the disk's unlock. It is looked up when asked, and takes a moment.

The file is `/var/lib/amethystora/start-times/seconds.tsv`: a date and a number on each line, no time of day, kept on this
computer and never sent. Only administrators can read it, since a calendar of when a computer was started shows when
somebody was away: other accounts get no card. The first line stays for good, with the rest of the first week, and the
latest starts; the file never holds more than 1000 lines.

## When a device does not work

A few minutes after each start, the machine looks for devices whose driver is missing, refused or
without its firmware, and says each once, with what fixes it where something does. To see what it
found:

```bash
ame system drivers          # as the last check found it
sudo ame system drivers     # looked at now
```

A module refused while Secure Boot is on means its key is not enrolled ([Secure Boot](#secure-boot)).
Firmware the image lacks is often a device too new for the image's kernel, which a later update may
bring. `ame report` puts what the check found in a bug report.

`ame system bios-info` prints the board's maker and model and the firmware's version and date, which
a question about a device often needs. `ame system benchmark` keeps every processor busy for a minute
with stress-ng's matrix test and prints what it got through, to compare machines or to see that one
stays stable under load; it offers to install stress-ng from Homebrew first.

## Drawing tablets

Most drawing tablets work as soon as they are plugged in, and GNOME's Settings has a page for one once
it is. For a tablet the kernel's own drivers handle badly, which is often the case for Huion, XP-Pen
and Gaomon models, or to set every tablet up the same way, use OpenTabletDriver:

```bash
ame apps opentabletdriver on
```

It installs OpenTabletDriver from Flathub, copies its rules for which devices it may read into `/etc`,
from the release Amethystora pins and checked against that release's checksum, and starts its driver
at each login. It also turns off the kernel's own tablet drivers, which would take the tablet first,
so restart if the tablet was plugged in. It moves the pointer through `/dev/uinput`, which the image keeps
from apps, so it gives that back to whoever sits at the machine ([Switches](security.md#switches), `ame
security virtual-input`). Set the pen's pressure, its buttons and the part of the screen
the tablet covers in **OpenTabletDriver**, in the app grid. `ame apps opentabletdriver off` takes it all
away again, and the kernel's drivers come back after a restart. The **Drawing tablets** switch on
[Control](control.md)'s **Apps** page does either in a terminal.

## Laptops

Framework laptops get their known fixes at first boot: the keyboard on the Intel models, which turns
the light sensor off too and restarts the machine once by itself; suspend on a Laptop 13 with AMD
Ryzen 7040 and firmware older than 3.09, whose own keyboard then no longer wakes it; natural scrolling
for a mouse; and a larger text size on a Laptop 13 with its original 2256×1504 screen. A Framework 16's keyboard and
numpad modules cannot wake it. Keep the firmware current on every machine: a
notification says when an update is waiting, and **Updates** shows what there is, or in a terminal:

```bash
fwupdmgr get-updates
fwupdmgr update
```

[Firmware](updates.md#firmware) says more.

`ame system power-draw` measures what the machine draws while idle, with powerstat: run it on
battery, and leave the machine alone for the few minutes it takes.

## Cameras

Virtual cameras (OBS's, for instance) work without anything to install, through the loopback
module built into the image.
