# Installing

Download Amethystora, write it to a USB stick, start the computer from the stick and try it: nothing on
the computer changes until you choose to install. Installing takes about ten minutes and needs no network.

![From download to a running desktop](media/usb-stick-boot.webm)

## Download

Each [image](choosing.md#pick-an-image) has two ISOs. **Try and install** is Amethystora on a stick: try
it, then install from there. Start with that one. **The installer alone** also starts computers too old
for UEFI. Each is several gigabytes.

| Image | Try and install | The installer alone |
| --- | --- | --- |
| The standard desktop | [amethystora.iso](https://download.amethystora.org/amethystora.iso) | [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso) |
| [Developer mode](developer.md) | [amethystora-dx.iso](https://download.amethystora.org/amethystora-dx.iso) | [amethystora-dx-non-uefi.iso](https://download.amethystora.org/amethystora-dx-non-uefi.iso) |
| The standard desktop with NVIDIA's driver | [amethystora-nvidia-open.iso](https://download.amethystora.org/amethystora-nvidia-open.iso) | [amethystora-nvidia-open-non-uefi.iso](https://download.amethystora.org/amethystora-nvidia-open-non-uefi.iso) |
| Developer mode with NVIDIA's driver | [amethystora-dx-nvidia-open.iso](https://download.amethystora.org/amethystora-dx-nvidia-open.iso) | [amethystora-dx-nvidia-open-non-uefi.iso](https://download.amethystora.org/amethystora-dx-nvidia-open-non-uefi.iso) |
| The standard desktop with NVIDIA's driver for the GeForce 900 and 10 series | [amethystora-nvidia-lts.iso](https://download.amethystora.org/amethystora-nvidia-lts.iso) | [amethystora-nvidia-lts-non-uefi.iso](https://download.amethystora.org/amethystora-nvidia-lts-non-uefi.iso) |
| Developer mode with NVIDIA's driver for the GeForce 900 and 10 series | [amethystora-dx-nvidia-lts.iso](https://download.amethystora.org/amethystora-dx-nvidia-lts.iso) | [amethystora-dx-nvidia-lts-non-uefi.iso](https://download.amethystora.org/amethystora-dx-nvidia-lts-non-uefi.iso) |

### Check the download

Beside each ISO is its checksum, `.sha256`, signed with the key every update is checked against. The
checksum alone is not enough: it comes from the same server as the ISO, and only the signature says that
Amethystora made it. For another ISO, put its name in place of `amethystora.iso`:

```bash
curl -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl -LO https://download.amethystora.org/amethystora.iso.sha256
curl -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
cosign verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
sha256sum -c amethystora.iso.sha256
```

On Windows, the same in PowerShell, in the ISO's folder, with cosign's own Windows build. The last line
prints `True` when the ISO matches:

```powershell
curl.exe -LO https://github.com/sigstore/cosign/releases/latest/download/cosign-windows-amd64.exe
curl.exe -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl.exe -LO https://download.amethystora.org/amethystora.iso.sha256
curl.exe -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
.\cosign-windows-amd64.exe verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
(Get-FileHash amethystora.iso).Hash -eq (Get-Content amethystora.iso.sha256).Split(' ')[0]
```

### With an NVIDIA graphics card

Take an ISO with NVIDIA's driver. With Secure Boot on, the first start after installing begins with a
blue screen, the MOK manager, which asks you to trust the key the driver is signed with
([Hardware](hardware.md#secure-boot)). Installed from another ISO, a notification after the first boot
offers the image with NVIDIA's driver ([Hardware](hardware.md#graphics)).

## Write it to a USB stick

Use a stick of 16 GB or more: everything on it is erased.

- **From Windows or macOS:** Fedora Media Writer (choose **Select .iso file**) or balenaEtcher. On
  Windows, Rufus works too, in DD image mode.
- **From Linux:** Impression, or `dd`, where `/dev/sdX` is the stick as `lsblk` lists it. Get this wrong
  and `dd` erases another disk:

  ```bash
  sudo dd if=amethystora.iso of=/dev/sdX bs=4M status=progress oflag=sync
  ```

A stick made with Ventoy starts the ISO only in Ventoy's GRUB2 mode.

## Try it

Plug the stick in and start the computer from it: press its boot menu key as it starts (F12, F11, F10, F8
or Esc on most PCs; hold Option on an Intel Mac) and choose the stick. Secure Boot can stay on. Choose
**Try or install Amethystora**, or **basic graphics mode** if the screen stays black. An installed system
takes basic graphics off at its first start, and restarts once by itself to do so.

Amethystora starts as it will once installed, already logged in, so Wi-Fi, sound, the display and the
rest show whether this computer suits it. Nothing is written to the computer's disk, and whatever you do
is gone when it shuts down. The apps from Flathub are not on the stick: they install with the first start
of the installed system.

The live ISO starts only computers with UEFI firmware, which nearly every computer since 2012 has. For an
older one, use the installer alone.

## Install

Choose **Install to Hard Drive** in the dash. The installer asks for your language and the disk, then gets
on with it.

- **Choose disk encryption.** It cannot be turned on afterwards, and it keeps your files safe if the
  computer is lost or stolen. The passphrase is asked at every start, until
  [the TPM unlocks it](disk-encryption.md#unlock-the-disk-with-the-tpm) if you want it to.
- **To keep Windows,** read [Next to Windows](#next-to-windows) first.
- **Choose one disk.** Two disks chosen here hold the system with one copy of each file, so either one
  failing loses everything. Install on one, then [mirror the system](disks.md#the-systems-own-disks) onto
  a second disk, or make a [pool](disks.md#pools-of-disks-raid) of the others. If the system is on two
  already, Control's **Protect the system's disks** keeps a copy on each.

When it is done, restart and take the stick out. The first start asks for your language, keyboard,
account and password, and the apps install by themselves once the computer is online. Then go through
[Getting started](getting-started.md).

## Next to Windows

Amethystora installs beside Windows, and a menu at each start lets you choose one. In Windows, before you
install:

1. **Keep your BitLocker recovery key.** Most Windows 11 laptops encrypt their disk with BitLocker (also
   called device encryption). Find the key under **Settings › Privacy & security › Device encryption**,
   or at [aka.ms/myrecoverykey](https://aka.ms/myrecoverykey), and keep it somewhere other than this
   computer: installing adds an entry to the boot menu, and Windows may ask for the key once afterwards.
2. **Make room.** Right-click **Start**, open **Disk Management**, right-click the Windows partition (C:)
   and choose **Shrink Volume**. Leave at least 64 GB free, more if you will keep your files there.
3. **Turn off Fast Startup:** **Control Panel › Power Options › Choose what the power buttons do**, and
   clear **Turn on fast startup**. With it on, Windows only half shuts down and keeps its disk locked.

Then, in the installer, install into the free space you made, never onto the whole disk. Keep Secure Boot
on: Windows 11 needs it, and Amethystora starts with it. If Windows' clock is a few hours off after you
have used Amethystora, turn on **Set time automatically** in Windows' date and time settings.

## From another Fedora Atomic desktop

A computer running Fedora Silverblue, or another desktop built as an image on Fedora, switches to
Amethystora without reinstalling. The switch replaces the operating system image and keeps everything in
`/home`, `/etc` and `/var`.

- **Back up** anything you cannot lose. Switching is safe, but a backup always is.
- Packages layered with `rpm-ostree install` do not come along. Flatpaks, Homebrew and containers do.

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Then restart. On the first boot a service stages the switch to signature-checked updates by itself, and
from the restart after that, an update is accepted only if it carries Amethystora's signature.

## After the first boot

Go through [Getting started](getting-started.md). The one step not to skip is enrolling the Secure Boot
key, if Secure Boot is on.
