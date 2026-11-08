# Picking an image

Amethystora comes as six images, each in three streams. Most people want the standard desktop on
`stable`, which is what the ISOs install; you can move between images and streams at any time.

## Pick an image

| Image | For |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | The standard desktop |
| `ghcr.io/arsslensoft/amethystora-dx` | Developers: adds Docker, Incus, libvirt, VSCodium and more ([Developer mode](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | The standard desktop with NVIDIA's open kernel driver |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Developer mode with NVIDIA's open kernel driver |
| `ghcr.io/arsslensoft/amethystora-nvidia-lts` | The standard desktop with NVIDIA's driver for the GeForce 900 and 10 series |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-lts` | Developer mode with NVIDIA's driver for the GeForce 900 and 10 series |

Each has its own ISOs ([Download](installing.md#download)). On a computer with an NVIDIA card installed
without the driver that drives it, the first boot offers to switch to the NVIDIA image that does
([Hardware](hardware.md#graphics)).

## Pick a stream

The tag after the colon sets the stream:

| Tag | For |
| --- | --- |
| `stable` | Everyone. Built weekly, with the kernel held back to Fedora CoreOS's, which lags a little behind Fedora's so that kernel regressions are caught first. |
| `latest` | The newest Fedora has, rebuilt whenever Amethystora changes. |
| `beta` | Testing what comes next. Expect rough edges. |

The ISOs install `stable`. [Updates](streams.md#switching-streams-and-images) shows how to switch.

## Sealed images

A sealed image starts only if every file of the system is exactly what was signed. Its kernel, the
kernel's settings and the digest of all its files are one signed file, which Secure Boot checks, and each
file is checked against that digest whenever it is read: a changed file cannot be read at all.
[Updates](streams.md#sealed-images) explains what that changes. Sealed images are new and experimental,
offered beside the regular images rather than instead of them.

### Before you choose one

- **Only a new installation makes one.** A regular installation cannot switch to a sealed image, nor a
  sealed one back, without installing again.
- **Moving from a regular installation** goes through a backup: [Moving to a sealed
  image](streams.md#moving-to-a-sealed-image).
- **It takes a whole disk.** The sealed installer erases one disk, and cannot install beside Windows yet.
- **It needs a network connection.** The installer downloads the system, about 5 GB.
- **Keep Secure Boot on.** Without it the files are still checked, but nothing checks the kernel that
  holds their digest.

### Installing one

Start the live ISO, [amethystora.iso](https://download.amethystora.org/amethystora.iso), and open
**Install Amethystora, sealed (experimental)** from the app grid. It asks which image and which disk, then
for a disk passphrase: a sealed system is always encrypted, since the seal covers the system's files and
not yours. The passphrase is typed before the system starts, on a US (QWERTY) keyboard layout whatever
yours is, so choose one whose characters are where you expect them on that layout.

The next start begins with the blue MOK manager, which asks you to trust Amethystora's key: choose
**Enroll MOK**, **Continue** and **Yes**, type `amethystora` and choose **Reboot**. Then comes the disk
passphrase, and the first start's setup creates your account.

The sealed images are the regular four, tagged `stable-sealed`:
`ghcr.io/arsslensoft/amethystora:stable-sealed`, `ghcr.io/arsslensoft/amethystora-dx:stable-sealed`, and
the same for the two NVIDIA images.
