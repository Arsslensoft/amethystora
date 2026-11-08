# Moving in

Ways to bring your things from another computer, and to send a file to someone far away. Each says what
it reads, what it copies and what it never touches, and none sends anything until you start it. For a phone
or another computer on your own network, see [Send to nearby](phone-sharing.md#send-to-nearby).

## Move to this computer

Your folders and your setup, from an old computer to this one over the network, with nothing in between:
no account, no cloud. Both computers run Amethystora on a network you named home (see
[Security](security.md)), with a person at each.

![Two computers, the same digits, then the folders](media/move-two-computers.webm)

On **this**, the new computer: Control's **System** page, **Move to this computer**. On the **old** one:
**Move this computer's things to a new one**. Or in a terminal:

```bash
ame system move receive                       # on the new computer
ame system move send                          # on the old one; it asks which if it finds several
ame system move send --to 192.168.1.20        # or call one directly
```

### The six digits

Both screens show the same six digits. **Compare them by eye**: if they differ, answer no on either side
and nothing moves. They prove that nobody sits between the two computers; they are not a password, and
nothing of them is kept.

### What you choose

The new computer lists what the old one has:

- **The home folder**: the folders you made yourself (Documents, Pictures, Music, Videos, Downloads,
  Desktop and any other), your Flatpak apps' data (`~/.var/app`, less their caches), and the Vault's own
  backup files, which open only with their recovery key. Those land in `~/From old computer/Vault backups`,
  not in this computer's own backup folder, which keeps the seven newest and deletes the rest. To bring the Vault's
  contents, restore the newest with `ame vault restore "$HOME/From old computer/Vault backups/FILE"` and
  your recovery key, **before** you wipe the old computer.
- **The setup file**, naming the apps, theme and shortcuts (`ame setup save` makes it). It lands in
  `~/From old computer`; `ame setup apply "$HOME/From old computer/setup.json"` shows what it would change
  and asks first.

### What it never copies

Browser profiles, saved logins, cookies, keyrings, `~/.ssh` and `~/.gnupg`, the Private folder, every
hidden folder not named above, and places that start programs (`~/bin`, `autostart`). Links and devices
stay behind. The new computer refuses these itself, whatever the other end sends.

### How it copies

- **It adds and never deletes.** A file whose name is already here is kept as it is, and the new computer
  says how many.
- **No half files.** A file appears under its name only when whole; run it again after a stop and it
  carries on with what is missing.
- **It stays out of the way:** lowest disk and processor priority, and a gigabyte left free. A home that
  does not fit is left out, and said; a sender that sends more than it said, or than there is room for,
  is stopped.

### How it is kept private

The key is agreed for this move alone (P-256, each direction sealed with AES-256-GCM), lives in memory and
is gone when the command ends. Only a computer on a network named home can reach the other: the firewall
opens port 4793 in that zone alone, the new computer listens on it only while the command waits, and each
side checks the zone again before it speaks. A move needs a terminal and cannot be started by
[the agent](ai-agent.md).

## Bring everything from your old computer

The old computer need not run: plug in its disk (an internal one in a case or a dock, or an external
drive) and choose **Bring everything from your old computer** on Control's **System** page, or:

```bash
ame system bring-over
```

![A disk plugged in, its folders copied home](media/bring-over-disk.webm)

It lists the disks that look like another computer's, never this computer's own. Pick one; if it is
encrypted with BitLocker, type its recovery key (48 digits; nothing shows as you type). It then shows each
person on the disk with their folders and sizes, and you choose:

- **Documents, Pictures, Music, Videos, Desktop and Downloads** go to the same folders here, in your own
  language's names.
- **Browser bookmarks** become one HTML file, `Bookmarks from old computer.html` in Documents, which any
  browser imports.

### Disks it reads

Windows, macOS and Linux disks: NTFS, exFAT, FAT, ext4, btrfs, XFS and HFS+, as this computer's kernel has
them. An APFS disk cannot be read. A LUKS-encrypted one has to be unlocked in Files first, then
`ame system bring-over --from /run/media/you/NAME`.

### What it promises

- **The disk is read-only.** It is mounted with `-o ro`, and an ext2/3/4 disk with `noload` too, so that
  its journal is not replayed onto it. Only disks you plugged in (removable or on USB) are offered, never
  this computer's own disks, pools or what `/etc/fstab` names. One the desktop mounted writable is
  unmounted and mounted again read-only, and all are unmounted (and locked) again at the end.
- **One exception, said first:** an XFS or btrfs disk that was not shut down cleanly may have its log
  replayed by the kernel when mounted, even read-only, which writes to it. udisks cannot prevent that, so
  for those two it asks first (default no). A disk Files mounted writable and handed over with `--from` is
  read as it is, and reading updates its access times.
- **There is room for it.** Before each folder it compares the size with the free space here and keeps a
  gigabyte free: a folder that does not fit is left out, and said.
- **It copies and never moves, and never replaces.** A file whose name is already here stays as it is.
  Each file appears only when whole; run it again after a stop and it carries on.
- **It never reads a password.** Of a browser it opens its bookmarks and nothing else: not `logins.json`,
  not Chrome's `Login Data`, not cookies, not keys. The recovery key goes to the disk's unlocker on a pipe,
  never an argument, an environment variable or a file, and is not kept once the disk is open. A script
  bookmark is dropped.
- **Nothing on the disk is trusted.** Links are never followed or copied; files become plain (folders 755,
  files 644, so a Windows disk's everything-executable goes); `desktop.ini`, thumbnails, lock files and the
  trash stay behind.
- It runs at the lowest disk and processor priority, so you can go on using the computer.

It is a command you start, not something that watches for disks: nothing runs until you do. The agent
cannot start it.

## Send to anyone

A file too big for email, to somebody far away: **three words** read out over the phone, and it arrives. No
account, and no link that lives on.

![Three words, and the file crosses](media/send-three-words.webm)

It is **offered and off**: turn it on with **Send to anyone** on Control's **Desktop** page, or
`ame desktop send-anyone on`. Files' **Scripts** menu on a file then has **Send to anyone**.

### Sending a file

The first time, it says where the file passes and asks before it fetches anything: the sending is
**Warp**, from Flathub (about 5 MB, plus the GNOME runtime if you do not have it), which uses Magic
Wormhole. Warp opens: press **Send File**, choose the file, and read the three words it shows to the other
person, who types them into Warp (or another Magic Wormhole app). Warp takes no file from outside, so you
choose it in its own window; the terminal says which.

### Where the file passes

- The three words are agreed through **a public server of the Magic Wormhole project**. It sees that two
  computers made contact, and their addresses. It cannot read the file.
- The file goes **straight to the other computer** when the two can reach each other, and otherwise
  through **a public relay of the same project**, which sees the two addresses and how much passes. It
  cannot read the file: it is encrypted from your computer to theirs.
- Nothing is kept: the words work once. Warp's own preferences name the servers and let you choose your
  own.

It is listed under [what this machine sends](security.md) once it is on or Warp is installed, and sends
nothing until you send a file in Warp's window. `ame desktop send-anyone off` takes the entry away; Warp,
if installed, stays until you remove it in Bazaar.
