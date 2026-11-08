# Streams

A stream is the track of builds your machine follows. You can switch between streams and images at any time, and every
update is checked against Amethystora's signature before it is installed.

## The streams

![Switching streams](media/streams-tracks.webm)

| Tag | What you get |
| --- | --- |
| `stable` | Built every week, with the kernel held back to Fedora CoreOS's, which lags a little behind Fedora's so that kernel regressions are caught first. For everyone. |
| `stable-daily` | The same, rebuilt whenever Amethystora changes rather than weekly |
| `latest` | The newest Fedora and kernel, rebuilt whenever Amethystora changes |
| `beta` | What comes next. Expect rough edges. |

## Switching streams and images

[Control](control.md)'s **System** page lists the images and streams: choosing one opens `ame system rebase` in a
terminal with it chosen. In a terminal, `ame system rebase amethystora-dx latest` starts with that image and stream
chosen. Either way it takes signed images only, asks before it switches, keeps layered packages, and refuses an NVIDIA
image while Secure Boot is on and the image's keys are not enrolled. It offers `stable`, `stable-daily` and `latest`;
only `bootc switch` reaches `beta`.

### Switching by hand

Switch to the image and restart:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Replace `amethystora` with `amethystora-dx` for [developer mode](developer.md), add `-nvidia-open` or `-nvidia-lts` for
NVIDIA's driver ([which one](hardware.md#graphics); with Secure Boot on, enroll the keys first), and `latest` with the
stream you want. A [sealed](#sealed-images) machine switches only between the sealed images.

With [layered packages](software.md#layering-the-last-resort), use `rpm-ostree` instead: `bootc switch` builds the new
system from the image alone and would drop them.

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Signed updates

An update is accepted only if it carries Amethystora's signature, checked against the key in
`/etc/pki/containers/amethystora.pub`. A tampered image, or one from anyone else, is refused. A service checks at every
boot that signature checking is still on, and turns it back on if a switch turned it off.

## Sealed images

On a [sealed image](choosing.md#sealed-images) the signature is not the last check. The kernel, its settings and the
digest of every file of the system are one file, a unified kernel image, signed with Amethystora's Secure Boot key. The
firmware starts shim, shim starts systemd-boot, and systemd-boot starts that kernel only if its signature holds.

### What the seal checks

The system is mounted only if its files match the digest, and each file is checked again whenever it is read. A file
changed on the disk, by a program running as root or by someone who took the disk out, reads as an error instead of
running. A disk changed so that its files cannot be checked at all does not start: a message says so, and the
[live ISO](installing.md#try-it) can still read your files. `/etc` and `/var` are your machine's own, and are not
covered.

### What is different on a sealed machine

- **Updates and rolling back work as on any other,** from System Updates, or with `sudo bootc upgrade` and
  `sudo bootc rollback`. There is no `rpm-ostree`, so nothing can be layered.
- **The kernel's settings are fixed.** They are inside the signed kernel image, so neither the boot menu nor anything on
  the machine can edit them. If it does not start, start the [live ISO](installing.md#try-it): it can unlock and read
  the disk.
- **It follows `stable-sealed`,** and switches only between the sealed images: `ame system rebase` offers no others.
- **Sealed and regular do not mix.** A regular installation keeps its system in ostree, a sealed one in composefs, and
  bootc cannot move a machine between them yet. Going either way means installing again, after a [backup](backups.md).
  That may change once bootc declares the sealed layout stable.
- **The boot loader is placed once,** by the installer. Updates bring a new kernel image each time, but not a new
  systemd-boot or shim.

### Moving to a sealed image

A regular installation cannot become a sealed one, so moving means installing again and bringing your things back from
a backup:

1. On the old system, make sure the [backup](backups.md) is set up and recent: **Back up now** in Backups. Then make a
   recovery key with `ame backup recovery-key` and write it down. The backup has to be somewhere other than this disk (a
   backup disk, an address, your other computer): the hourly snapshots are on the disk the installer erases.
2. If you use the [Vault](vault.md), have its recovery key at hand as well.
3. Install the sealed image from the live ISO, as [Installing](choosing.md#sealed-images) describes. It erases the disk.
   Create your account with the **same account name** as before: files come back only into the same home folder.
4. At the first look, choose **Have a backup?** (or run `ame backup restore-from`). It asks for the recovery key and
   brings back your files, your apps and setup, and your Vault, each step chosen first.
   [Starting from your backup](backup-other.md#starting-from-your-backup) says what each one does.

What does not come back: layered packages (a sealed machine has none) and changes to `/etc` made by hand. Security
settings are the machine's: set them again in Security.
