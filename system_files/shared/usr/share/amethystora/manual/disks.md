# Disks

The system warns you when a disk may be failing, and several disks can keep copies of each other so that one can die
without taking a file with it.

- **Encryption** is chosen when installing and cannot be added afterwards. Once it is on, `ame security disk-unlock`
  lets the TPM unlock the disk at boot instead of you typing the passphrase
  ([Security](disk-encryption.md#unlock-the-disk-with-the-tpm)).
- **Several disks** can keep copies of each other: [Pools of disks](#pools-of-disks-raid).
- **ZFS** is available on the `stable` stream.
- **Apple's USB SuperDrive** is woken up as it is plugged in, as a Mac would, so that it reads discs.

## When a disk may be failing

Disks usually say they are failing long before they do. Every hour the disks the system is on are asked for their
health (SMART); a disk asleep is left asleep. If one says it is failing, or starts counting bad sectors or media errors,
a notice says so once (again if the count rises), with **Back up now** and how long it has been since your last backup.
Bad sectors a disk already had when it was first read are not news: only a rise is.

### What to do

It may be wrong either way: a disk that reports errors can run for months, and one can die without a word. Take it as
the moment to copy what matters ([Backups](backups.md) is the quickest way), then replace the disk. A disk in
[a pool](#pools-of-disks-raid), the system's own included when it is on several disks, is told of by the pools' check
instead, so that one disk is not the subject of two notices.

### The switch and what is kept

`ame system disk-warning` says what each disk said last. `ame system disk-warning off` stops reading them, and nothing
then warns you; `on` starts again, both also in [Control](control.md). Kept in `/var/lib/amethystora/disk-health.json`,
which any account can read: per disk, its model, a checksum of its serial number (to tell disks apart when their names
swap), its current name, what it said (health and error count) and what was told of it.

## Pools of disks (RAID)

Two disks or more can hold the same files, so that one can fail without taking a file with it: that is a pool. The
**Disks** group on [Control](control.md)'s **System** page shows and makes them.

![One disk dies, the files stay](media/pool-disk-dies.webm)

In a terminal:

```bash
ame system raid              # how the pools are, and what can be done next
ame system raid create       # a pool from spare disks
```

### Making a pool

`create` offers only disks nothing is using: never the system's own, nor one with anything mounted or open on it. Pick
the disks, a layout and whether to encrypt, and type `ERASE`: everything on those disks is erased, and the list shows
what is on each first. Every question has its answer chosen already, so it can be Enter, Enter, Enter and `ERASE`. The
pool is then at `/var/mnt/data` (`data2` for a second one), belongs to you, and is in the Files sidebar.
`ame system raid create --name media /dev/sdb /dev/sdc` names the pool and its disks ahead: a name of up to 31
lower-case letters, digits and dashes, and two spare disks or more.

| Layout | From | Survives | Worth knowing |
| --- | --- | --- | --- |
| **Two copies** | 2 disks | one dead disk | The one to choose unless you have a reason not to |
| **Three copies** | 3 disks | two dead disks | A third of the space for files |
| **Striped two copies** | 4 disks | one dead disk | Faster for large files, and as safe as two copies |
| **Parity** | 3 disks | one dead disk | btrfs's own advice: not for production, only for testing. A power cut can damage what was being written: check every copy after one |
| **Double parity** | 4 disks | two dead disks | The same as parity |
| **Combined** | 2 disks | none | Joins the disks into one and keeps no copies: a dead disk takes its files with it |

The list of layouts says how much each would hold on the disks you picked.

### Worth knowing about a pool

- **Encrypted** unless you say no. The pool's key is kept on the system disk, so the pool opens with the system and is
  as safe as the system disk, and a pool disk that leaves the house gives nothing away. `create` shows a **recovery
  key** once: write it down. It opens the pool's disks on any Linux machine; without it, a pool whose system disk is
  lost is lost too.
- **A pool is not a backup.** A file deleted, or encrypted by ransomware, is gone from every disk at once.
  [Backups](backups.md) cover your home folder; to back up a pool too, add its folder to `AMETHYSTORA_BACKUP_PATHS` in
  `~/.config/amethystora/backup.env`, after your home folder.
- **A disk on USB** can be part of a pool, but a loose cable takes it out: keep it plugged in.

### When a disk fails

A pool that loses a disk goes on working, which is why nobody notices until the second one goes. So Amethystora looks
every hour and says so in a notification: which pool, which disk by model and serial number, and whether every file is
still there. **Open Control** goes to the Disks group, where the pool says what happened and offers:

- **Replace a disk** (`ame system raid replace NAME`) takes a spare disk at least as large, erases it and copies onto it
  what the old one held. It asks for the pool's recovery key to put on the new disk; left empty, every disk of the pool
  gets a new one. It goes on if the window is closed. btrfs needs a gigabyte free on each remaining disk to rebuild, so
  a pool too full for that is refused before anything is erased: delete what you can spare and try again. A replace
  that stops part way leaves the new disk blank, to be picked once more.
- **Open it with a disk missing** (`ame system raid mount NAME`) is for a pool that did not open because a disk was
  missing at start. What is written meanwhile has one copy until the disk is replaced. A pool missing more disks than
  its layout survives opens read-only, so that what is left can be copied off.
- **Put the copies back** comes up when the missing disk returns. What was written while it was away has one copy, and
  the disk still has the old copies: btrfs reads the good copy and counts the old one as an error, so the disk shows as
  **back**, not failing. This checks every copy, rewrites the old ones from the good, gives what has one copy its
  second, and sets the disk's count to zero. A file btrfs keeps no checksum of (one marked No_COW, such as the
  journal's) cannot be told apart that way, so its copy on that disk may stay old.
- A disk **counting errors** had reads or writes fail. btrfs read the other copy, so no file was lost, but a count that
  keeps rising means the disk, or its cable, is going. Once the cause is fixed,
  `sudo btrfs device stats --reset /var/mnt/NAME` sets the count back to zero.

### Looking after a pool

- **The monthly check** reads every copy, on mains power, and repairs a damaged one from a good one. **Check every copy
  now** (`ame system raid scrub now`) does it straight away: do that after a power cut on a pool with parity.
  `ame system raid scrub off` turns the monthly check off and `on` back on; `ame system raid scrub` says whether it is
  on and offers to change that.
- **Add a disk** (`ame system raid add NAME`, or `add NAME DISK` to name it) makes room on an open pool. It asks whether
  to spread what is already there over every disk now, which takes hours on a large pool.
- **Forget** (`ame system raid forget NAME`) stops mounting a pool and leaves its files on its disks. GNOME Disks'
  **Format** erases them when you want the disks back.

Without a `NAME`, each of these takes the only pool, or asks which.

### The system's own disks

The installer makes no pools: install on one disk, and make a pool of the others afterwards. The system itself can be on
two disks as well, so that the machine goes on with one dead:

- **Mirror the system onto a new disk** (`ame system raid mirror DISK`) copies a system on one disk onto a spare disk at
  least as large: its own EFI partition, a half of `/boot` (the two halves are a RAID1, `md-boot`), and a copy of every
  file, encrypted with the system's passphrase if the system is. It erases that disk once you type `ERASE`, and nothing
  on the system's disk. The machine then restarts once from the new disk; once that start has worked, the first disk's
  `/boot` joins the new one's, and the machine starts from either disk. The passphrase is still typed once, and a
  [TPM](disk-encryption.md#unlock-the-disk-with-the-tpm) that unlocked the disk gets a key for the new one too, with its
  PIN, as the command asks.
- **Protect the system's disks** (`ame system raid mirror`) is for a system the installer spread over two disks with one
  copy of each file, where either failing loses everything. It keeps a copy of each file on each disk instead, and
  erases nothing. The boot files stay on the first disk: the machine starts from it, and with that disk dead every file
  is still on the other, readable from the live USB stick.

A sealed machine cannot be mirrored: its signed kernel command line names one disk.

### Starting with a disk dead

Without one of its disks, the system waits for it at the start, with no end. Turn the machine off and on again, and in
the boot menu (shown for five seconds on a system over several disks) choose **Start with a disk missing**: the system
starts from the disk that is left, and says which is missing. Then **Replace a disk** (`ame system raid replace system`)
puts a new disk, at least as large, in its place, with its own EFI partition and half of `/boot`, and **Put the copies
back** mends what was written meanwhile once the disk itself comes back. If the firmware does not start from the second
disk by itself, choose it once in its boot menu (often F12 or F8).
