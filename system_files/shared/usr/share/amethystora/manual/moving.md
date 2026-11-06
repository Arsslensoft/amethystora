# Moving in

Ways to bring your things from another computer, and to send a file to someone far away. Each says what it reads, what it
copies and what it never touches, and none of them sends anything until you start it. To send a file to a phone or another
computer on your own network, see [Send to nearby](phone.md#send-to-nearby).

## Move to this computer

Your folders and your setup, from an old computer to this one over the network, with nothing in between: no account, no
cloud. Both computers run Amethystora, on a network you named home (see [Security](security.md)), and a person sits at each.

On **this** computer, the new one, in Control's **System** page choose **Move to this computer**, or in a terminal:

```bash
ame system move receive
```

On the **old** computer, **Move this computer's things to a new one**, or:

```bash
ame system move send
```

If the old computer finds more than one, it asks which; to call one directly, `ame system move send --to 192.168.1.20`.
Both screens then show the same six digits. **Compare them by eye**: if they differ, answer no on either side and nothing
moves. They prove that nobody sits between the two computers; they are not a password, and nothing of them is kept.

The new computer then lists what the old one has, and you choose:

- **The home folder**: the folders you made yourself (Documents, Pictures, Music, Videos, Downloads, Desktop and any other),
  the data of your Flatpak apps (`~/.var/app`, less their caches), and the Vault's own backup file, which opens only with its
  recovery key. To bring the Vault's contents, restore that file with `ame vault restore` and your recovery key.
- **The setup file**, which names the apps, theme and shortcuts (`ame setup save` makes it). It lands in `~/From old computer`;
  bring it in with `ame setup apply "$HOME/From old computer/setup.json"`, which shows what it would change and asks first.

What it **never** copies: browser profiles, saved logins, cookies, keyrings, `~/.ssh` and `~/.gnupg`, the Private folder,
every hidden folder not named above, and places that start programs (`~/bin`, `autostart`). Links and devices stay behind.
The new computer refuses these itself, whatever the other end sends.

It **adds and never deletes**: a file whose name is already on this computer is kept as it is, and the new computer says how
many. A file appears under its name only when it is whole, so stopping half way leaves no half files, and running it again
carries on with what is missing. It runs at the lowest disk and processor priority, so the desktop stays quick, and leaves a
gigabyte free: a home that does not fit is left out, and said.

The key is agreed for this move alone (P-256, each direction sealed with AES-256-GCM), lives in memory and is gone when the
command ends. Only a computer on a network named home can reach the other: the firewall opens port 4793 in that zone alone, the
new computer listens on it only while the command waits, and each side checks the zone again before it speaks. A move needs a terminal and
cannot be started by [the agent](ai-agent.md).

## Bring everything from your old computer

You do not need the old computer to be running: plug in its disk (an internal one in a case or a dock, or an external drive) and
Control's **System** page has **Bring everything from your old computer**, or in a terminal:

```bash
ame system bring-over
```

It lists the disks that look like another computer's, never this computer's own. Pick one; if it is encrypted with BitLocker,
type its recovery key (48 digits; nothing shows as you type). It then shows each person on the disk and their folders with their
sizes, and you choose: **Documents, Pictures, Music, Videos, Desktop and Downloads** go to the same folders here, in your own
language's names, and **browser bookmarks** become one HTML file, `Bookmarks from old computer.html` in Documents, which any
browser imports. Windows, macOS and Linux disks are read: NTFS, exFAT, FAT, ext4, btrfs, XFS and HFS+ as this computer's kernel
has them. An APFS disk cannot be read, and a LUKS-encrypted one has to be unlocked in Files first (then
`ame system bring-over --from /run/media/you/NAME`).

What it promises, and how it is kept:

- **The disk is read-only and nothing on it changes.** It is mounted with `-o ro`; a disk the desktop already mounted so that it
  can be written to is unmounted and mounted again read-only. It is unmounted (and locked) again at the end.
- **It copies and never moves, and never replaces.** A file whose name is already here stays as it is. Each file appears under its
  name only when whole. If it stops, run it again: it carries on where it stopped.
- **It never reads a password.** Of a browser it opens its bookmarks and nothing else: not `logins.json`, not Chrome's `Login Data`,
  not cookies, not keys. The recovery key goes to the disk's unlocker on a pipe: never an argument, an environment variable
  or a file, and not kept once the disk is open. A script bookmark is dropped.
- **Nothing on the disk is trusted.** Links, on the disk or in a folder, are never followed or copied; files become plain (folders
  755, files 644, so a Windows disk's everything-executable goes); `desktop.ini`, thumbnails, lock files and the trash stay behind.
- It runs at the lowest disk and processor priority, so you can go on using the computer.

It is a command you start, not something that watches for disks: nothing runs until you do. The agent cannot start it.
