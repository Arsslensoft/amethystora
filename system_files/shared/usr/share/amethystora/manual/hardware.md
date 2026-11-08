# Hardware

Amethystora runs best on hardware Linux supports well: Intel and AMD graphics, and laptops their makers certify for
Linux. Most things work the moment they are plugged in; this page is about the ones that need a step from you.

## Secure Boot

Leave Secure Boot on. With it, the machine only boots a kernel it has checked, and the kernel's
[lockdown](security.md) has something to stand on.

The kernel modules built into the image (for DisplayLink docks, virtual cameras, Xbox controllers over Bluetooth or
Microsoft's wireless adapter, and NVIDIA's driver on the NVIDIA images) are signed with the image's own keys, which the
firmware has to be told to trust once:

```bash
ame security secure-boot
```

### The blue screen

Restart, and a blue screen appears before the system starts: the MOK manager. Choose **Enroll MOK**, then
**Continue**, then **Yes**, and type the password `amethystora`. The screen uses a US (QWERTY) layout whatever your own
is, which matters if your keyboard puts the letters elsewhere. `ame security status` tells you whether the keys are
enrolled.

If Secure Boot is off, a notification says so once. Turn it on in the firmware settings (`ame system bios` restarts
into them), then run the command above.

A [sealed image](streams.md#sealed-images) needs Amethystora's key before it can start at all, so its installer queues
it with the modules' key: the blue screen comes at the first start, with the same password. An NVIDIA image installed
from its ISO with Secure Boot on does the same, since NVIDIA's driver does not load without the key.

## Graphics

**Intel and AMD** work out of the box; the developer image adds ROCm for GPU compute on AMD. **NVIDIA** needs one of
the NVIDIA images ([Installing](choosing.md#pick-an-image)), which carry NVIDIA's driver. Which one depends on the card:

| Card | Image |
| --- | --- |
| GeForce 16 and RTX 20 series and newer (Turing on) | `-nvidia-open`, NVIDIA's open driver |
| GeForce 900 and 10 series, the GTX 750, Titan V (Maxwell, Pascal, Volta) | `-nvidia-lts`, NVIDIA's closed driver of its 580 branch, the last that drives them |
| Older (Kepler and before) | none: the image without NVIDIA's driver, whose open driver nouveau draws on them |

### NVIDIA and Secure Boot

NVIDIA's driver comes built for the image's kernel and signed with the image's own key, which the kernel trusts only
with Secure Boot on. So the NVIDIA images turn kernel lockdown on only with Secure Boot, as the kernel itself does,
never with an argument of their own. With Secure Boot on, enroll the keys ([above](#secure-boot)) before switching to an
NVIDIA image: `ame system rebase` offers to, and switches only once they are, since the screen would otherwise stay
black.

### The wrong image for the card

A few minutes after the first boot, a notification offers **Switch images…** when the image's driver does not drive the
card. It opens `ame system rebase` on the image that does: `-nvidia-open` or `-nvidia-lts` on a machine with an NVIDIA
card, or, on an NVIDIA image with no card NVIDIA's driver drives, the image without it, which keeps lockdown on. It
takes signed images only and asks before it switches. Nothing switches by itself, it is said once per machine, and
**Don't ask again** stops it for good.

## Memory

When RAM runs short, what programs are not using moves into compressed memory (zram) rather than to the disk, and comes
back when a program needs it. It is compressed quickly as it leaves RAM, and what stays unused is repacked smaller every
hour on mains power. On a 4 GB machine holding 3 GB of program data, that took about 420 MB of RAM per GB moved out,
against 540 MB with quick compression alone, and brought it back sooner than compressing small from the start.

### The kernel's memory settings

They are left as they are. The values often suggested for zram (`vm.swappiness` at 180, `vm.page-cluster` at 0, and the
two watermark settings) were measured on 4 GB and 8 GB machines, together and one at a time: none made the desktop
quicker, and a swappiness of 180 made it slower. Performance mode in the quick settings swaps less (a swappiness of 10),
which did no harm.

`ame system zram`, or **Swapped-out memory** on [Control](control.md)'s **System** page, explains the choices and asks:
`default`, `fast` (no repacking, Fedora's own) or `compact` (small from the start, slower to come back). A choice takes
effect from the next restart.

## How long it takes to start

Computers are expected to slow with age. This one replaces its system whole with every update, so the system itself
cannot wear out, and it keeps the numbers to show it: one per start, by date, on **Control › System**, with a graph.

```bash
ame system start-times      # what it measured
ame system start-times why  # when it got slower: what changed
ame system start-times off  # stop measuring; what was measured is kept
```

### What is measured

The time from the kernel starting to the system having started, login screen included, on the days it looks bad as well.
Unlocking the disk is left out (the time you take to type a password is not the computer being slow), and so are the
firmware and the boot menu. A [soft reboot](restarting.md#restarting-in-seconds) after an update is not a start.

### What it says

"Still as fast as day one" only for a computer measured since its first days; one updated into this says "Still as fast
as when it began to be measured", and since when. A tenth longer than in the first week and at least a second is "a
little more slowly"; a quarter longer and at least three seconds is "more slowly", here, in `ame system doctor` and
under Something's wrong. **What changed** (a button on the card, or `ame system start-times why`) names the update it got
slower with, if any, and what took longest in this start, leaving out the disk's unlock. It is looked up when asked, and
takes a moment.

### Where it is kept

`/var/lib/amethystora/start-times/seconds.tsv`: a date and a number per line, no time of day, kept on this computer and
never sent. Only administrators can read it, since a calendar of starts shows when somebody was away: other accounts get
no card. The first line stays for good, with the rest of the first week, and the latest starts; the file never holds
more than 1000 lines.

## When a device does not work

A few minutes after each start, the machine looks for devices whose driver is missing, refused or without its firmware,
and says each once, with what fixes it where something does.

```bash
ame system drivers          # as the last check found it
sudo ame system drivers     # looked at now
```

A module refused while Secure Boot is on means its key is not enrolled ([Secure Boot](#secure-boot)). Missing firmware
is often a device too new for the image's kernel, which a later update may bring. `ame report` puts what the check
found in a bug report.

### Machine details and a stress test

- `ame system bios-info` prints the board's maker and model and the firmware's version and date, which a question about
  a device often needs.
- `ame system benchmark` keeps every processor busy for a minute with stress-ng's matrix test and prints what it got
  through, to compare machines or check one stays stable under load; it offers to install stress-ng from Homebrew first.
