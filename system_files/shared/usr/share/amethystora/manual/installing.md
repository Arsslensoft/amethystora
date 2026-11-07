# Installing

Download Amethystora, write it to a USB stick, start the computer from the stick and try it: nothing
on the computer changes until you choose to install. Installing takes about ten minutes and needs no
network.

## Download

Each [image](#pick-an-image) has two ISOs. The first is Amethystora on a stick: try it, then install
it from there. Start with that one. The second is the installer alone, which also starts computers too
old for UEFI.

| Image | Try and install | The installer alone |
| --- | --- | --- |
| The standard desktop | [amethystora.iso](https://download.amethystora.org/amethystora.iso) | [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso) |
| [Developer mode](developer.md) | [amethystora-dx.iso](https://download.amethystora.org/amethystora-dx.iso) | [amethystora-dx-non-uefi.iso](https://download.amethystora.org/amethystora-dx-non-uefi.iso) |
| The standard desktop with NVIDIA's driver | [amethystora-nvidia-open.iso](https://download.amethystora.org/amethystora-nvidia-open.iso) | [amethystora-nvidia-open-non-uefi.iso](https://download.amethystora.org/amethystora-nvidia-open-non-uefi.iso) |
| Developer mode with NVIDIA's driver | [amethystora-dx-nvidia-open.iso](https://download.amethystora.org/amethystora-dx-nvidia-open.iso) | [amethystora-dx-nvidia-open-non-uefi.iso](https://download.amethystora.org/amethystora-dx-nvidia-open-non-uefi.iso) |
| The standard desktop with NVIDIA's driver for the GeForce 900 and 10 series | [amethystora-nvidia-lts.iso](https://download.amethystora.org/amethystora-nvidia-lts.iso) | [amethystora-nvidia-lts-non-uefi.iso](https://download.amethystora.org/amethystora-nvidia-lts-non-uefi.iso) |
| Developer mode with NVIDIA's driver for the GeForce 900 and 10 series | [amethystora-dx-nvidia-lts.iso](https://download.amethystora.org/amethystora-dx-nvidia-lts.iso) | [amethystora-dx-nvidia-lts-non-uefi.iso](https://download.amethystora.org/amethystora-dx-nvidia-lts-non-uefi.iso) |

Each is several gigabytes. Beside each ISO is its checksum, `.sha256`, signed with the key every
update is checked against. To check that what you downloaded is what was built (for another ISO, put
its name in place of `amethystora.iso`):

```bash
curl -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl -LO https://download.amethystora.org/amethystora.iso.sha256
curl -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
cosign verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
sha256sum -c amethystora.iso.sha256
```

On Windows, the same in PowerShell, in the folder the ISO is in, with cosign's own Windows build. The
checksum alone is not enough: it comes from the same server as the ISO, and only the signature says
that Amethystora made it. The last line prints `True` when the ISO matches:

```powershell
curl.exe -LO https://github.com/sigstore/cosign/releases/latest/download/cosign-windows-amd64.exe
curl.exe -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl.exe -LO https://download.amethystora.org/amethystora.iso.sha256
curl.exe -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
.\cosign-windows-amd64.exe verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
(Get-FileHash amethystora.iso).Hash -eq (Get-Content amethystora.iso.sha256).Split(' ')[0]
```

On a computer with an NVIDIA graphics card, take an ISO with NVIDIA's driver. With Secure Boot on, the
first start after installing begins with a blue screen, the MOK manager, which asks you to trust the
key the driver is signed with ([Hardware](hardware.md#secure-boot)). Installed from another ISO, a
notification after the first boot offers the image with NVIDIA's driver ([Hardware](hardware.md#graphics)).

## Write it to a USB stick

Use a stick of 16 GB or more: everything on it is erased.

- **From Windows or macOS:** Fedora Media Writer (choose **Select .iso file**) or balenaEtcher. On
  Windows, Rufus works too, in DD image mode.
- **From Linux:** Impression, or `dd`:

  ```bash
  sudo dd if=amethystora.iso of=/dev/sdX bs=4M status=progress oflag=sync
  ```

  where `/dev/sdX` is the stick, as `lsblk` lists it. Get this wrong and `dd` erases another disk.

A stick made with Ventoy starts the ISO only in Ventoy's GRUB2 mode.

## Try it

Plug the stick in and start the computer from it: as it starts, press its boot menu key (F12, F11,
F10, F8 or Esc on most PCs; hold Option on an Intel Mac) and choose the stick. Secure Boot can stay
on. Choose **Try or install Amethystora**, or the **basic graphics mode** if the screen stays black. An
installed system takes basic graphics off at its first start, and restarts once by itself to do so.

Amethystora starts as it will once installed, already logged in. Wi-Fi, sound, the display and the
rest of the hardware show whether this computer suits it. Nothing is written to the computer's disk,
and whatever you do is gone when it shuts down. The apps from Flathub are not on the stick: they
install with the first start of the installed system.

The live ISO starts only computers with UEFI firmware, which nearly every computer since 2012 has. For
an older one, use the installer alone.

## Install

Choose **Install to Hard Drive** in the dash. The installer asks for your language and the disk, and
then gets on with it.

- **Choose disk encryption.** It cannot be turned on afterwards, and it is what keeps your files safe
  if the computer is lost or stolen. The passphrase is asked at every start, until
  [the TPM unlocks it](security.md#unlock-the-disk-with-the-tpm) if you want it to.
- **To keep Windows,** read [Next to Windows](#next-to-windows) first.
- **Choose one disk.** Two disks chosen here hold the system with one copy of each file, so either one
  failing loses everything. Install on one, and afterwards [mirror the system](hardware.md#the-systems-own-disks)
  onto a second disk, or make a [pool](hardware.md#pools-of-disks-raid) of the others; if the system is
  on two already, Control's **Protect the system's disks** keeps a copy on each.

When it is done, restart and take the stick out. The first start asks for your language, keyboard,
account and password, and the apps install by themselves once the computer is online. Then go through
[Getting started](getting-started.md).

## Next to Windows

Amethystora installs beside Windows, and a menu at each start lets you choose one. In Windows, before
you install:

1. **Keep your BitLocker recovery key.** Most Windows 11 laptops encrypt their disk with BitLocker,
   also called device encryption. Find the key under **Settings › Privacy & security › Device
   encryption**, or at [aka.ms/myrecoverykey](https://aka.ms/myrecoverykey), and keep it somewhere
   other than this computer: installing adds an entry to the computer's boot menu, and Windows may ask
   for the key once afterwards.
2. **Make room.** Right-click **Start**, open **Disk Management**, right-click the Windows partition
   (C:) and choose **Shrink Volume**. Leave at least 64 GB free for Amethystora, more if you will keep
   your files there.
3. **Turn off Fast Startup:** **Control Panel › Power Options › Choose what the power buttons do**,
   and clear **Turn on fast startup**. With it on, Windows only half shuts down and keeps its disk
   locked.

Then, in the installer, install into the free space you made, never onto the whole disk. Keep Secure
Boot on: Windows 11 needs it, and Amethystora starts with it. If Windows' clock is a few hours off
after you have used Amethystora, turn on **Set time automatically** in Windows' date and time
settings.

## From another Fedora Atomic desktop

A computer that already runs Fedora Silverblue, or another desktop built as an image on Fedora,
switches to Amethystora without reinstalling. The switch replaces the operating system image and keeps
everything in `/home`, `/etc` and `/var`.

- **Back up** anything you cannot lose. Switching is safe, but a backup always is.
- Packages you layered onto the old system with `rpm-ostree install` do not come along. Flatpaks,
  Homebrew and containers do.

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Then restart. On the first boot a service stages the switch to signature-checked updates by itself,
and from the restart after that, an update is only accepted if it carries Amethystora's signature.

## Pick an image

| Image | For |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | The standard desktop |
| `ghcr.io/arsslensoft/amethystora-dx` | Developers: adds Docker, Incus, libvirt, VSCodium and more ([Developer mode](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | The standard desktop with NVIDIA's open kernel driver |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Developer mode with NVIDIA's open kernel driver |
| `ghcr.io/arsslensoft/amethystora-nvidia-lts` | The standard desktop with NVIDIA's driver for the GeForce 900 and 10 series |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-lts` | Developer mode with NVIDIA's driver for the GeForce 900 and 10 series |

Each has its own ISOs ([Download](#download)). On a computer with an NVIDIA card installed without the
driver that drives it, the first boot offers to switch to the NVIDIA image that does
([Hardware](hardware.md#graphics)).

## Pick a stream

Every image comes in three streams, set by the tag after the colon:

| Tag | For |
| --- | --- |
| `stable` | Everyone. Built weekly, with the kernel held back to Fedora CoreOS's, which lags a little behind Fedora's so that kernel regressions are caught first. |
| `latest` | The newest Fedora has, rebuilt whenever Amethystora changes. |
| `beta` | Testing what comes next. Expect rough edges. |

The ISOs install `stable`. You can move between images and streams at any time;
[Updates](updates.md#switching-streams-and-images) shows how.

## Sealed images

A sealed image starts only if every file of the system is exactly what was signed. Its kernel, the
kernel's settings and the digest of all its files are one signed file, which Secure Boot checks, and
each file is checked against that digest whenever it is read: a changed file cannot be read at all.
[Updates](updates.md#sealed-images) explains what that changes. Sealed images are new and
experimental, so they are offered beside the regular images rather than instead of them.

- **Only a new installation makes one.** A regular installation cannot switch to a sealed image, nor a
  sealed one back, without installing again.
- **Moving from a regular installation** goes through a backup: [Moving to a sealed
  image](updates.md#moving-to-a-sealed-image).
- **It takes a whole disk.** The sealed installer erases one disk, and cannot install beside Windows
  yet.
- **It needs a network connection.** The installer downloads the system, about 5 GB.
- **Keep Secure Boot on.** Without it the files are still checked, but nothing checks the kernel that
  holds their digest.

To install one, start the live ISO, [amethystora.iso](https://download.amethystora.org/amethystora.iso),
and open **Install Amethystora, sealed (experimental)** from the app grid. It asks which image and
which disk, then for a disk passphrase: a sealed system is always encrypted, since the seal covers the
system's files and not yours. The passphrase is typed before the system starts, on a US
(QWERTY) keyboard layout whatever yours is, so choose one whose characters are where you expect them on
that layout.

The next start begins with the blue MOK manager, which asks you to trust Amethystora's key: choose
**Enroll MOK**, then **Continue** and **Yes**, type `amethystora` and choose **Reboot**. Then comes the
disk passphrase, and the first start's setup creates your account, as after any installation.

The sealed images are the regular four, tagged `stable-sealed`:
`ghcr.io/arsslensoft/amethystora:stable-sealed`, `ghcr.io/arsslensoft/amethystora-dx:stable-sealed`, and the
same for the two NVIDIA images.

## After the first boot

Go through [Getting started](getting-started.md). The one step not to skip is enrolling the Secure
Boot key, if Secure Boot is on.
