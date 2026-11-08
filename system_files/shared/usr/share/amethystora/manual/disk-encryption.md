# Locked disks and folders

Two locks beyond your login: the TPM that unlocks the disk at boot behind a PIN, and a private folder that closes with
the screen.

## Unlock the disk with the TPM

Instead of typing the disk passphrase at every boot, the machine's TPM hands over the key, but only while Secure Boot is
on and the signing keys are the ones enrolled when you set it up, and only after you type a short PIN. Your passphrase
keeps working, so a cleared TPM never locks you out.

```bash
ame security disk-unlock         # set it up (with a PIN)
ame security disk-unlock on-pin  # replace a TPM key set up without a PIN
ame security disk-unlock off     # take the key out of the TPM; the passphrase is asked at every start
```

It needs a TPM and a disk that was encrypted when it was installed, and works from the next start. With Secure Boot off
it warns first: turn it on, and enroll the image's keys with `ame security secure-boot`
([Secure Boot](hardware.md#secure-boot)).

### Why the PIN

The PIN is what keeps the key from someone who takes the machine. On an image that is not sealed, the boot menu and the
programs that ask for the disk key are plain files: someone holding the machine could start it into a shell, or with a
disk of their own made to look like yours, and Secure Boot would see nothing changed. So it is set up only with a PIN
here, and the report's **Disk unlock** check warns about a TPM key set up without one, as earlier versions allowed.

On a [sealed image](streams.md#sealed-images) the programs that ask for the disk key are inside the signed kernel image,
so they cannot be swapped for others that would keep it. The PIN is still asked for there: Secure Boot does not see
which disk the signed system starts from, or the settings in `/etc` on it, and a disk of someone else's made to look
like yours would start the same system with theirs. Updates keep it working as long as Amethystora's key does not
change.

### A password on the boot menu

`ame security boot-password on` puts a password on the boot menu as well: editing a boot entry, which is how someone at
the keyboard would start the machine into a root shell, asks for it, and starting normally does not.

## A private folder that locks with the screen

**Private**, a folder in your home whose files are encrypted on their own, with a password of their own. It closes
whenever the screen locks, before the computer sleeps and when you log out, and its key leaves memory then. The rest of
your home stays readable to anyone who gets at the computer while it is on, locked or not, since the disk's own key has
to stay in memory for the system to run; this folder, while it is closed, does not.

![Closed when the screen locks](media/private-folder-lock.webm)

```bash
ame security private-folder           # the menu
ame security private-folder create    # asks for its password twice, then shows a recovery key, once
ame security private-folder open
ame security private-folder close
ame security private-folder password  # a new password, with the old one or the recovery key
ame security private-folder remove    # takes away an empty folder, once you type REMOVE
```

### Using it

- **Making it:** write the recovery key down and keep it away from the computer: it is the only way back in without the
  password.
- **Opening it:** **Private Folder** in the app grid asks for its password and shows it in Files, whose sidebar has it
  too. While it is closed it is empty, and nothing can be saved in it.
- **Closing it yourself:** **Close Now**, from the right-click menu of **Private Folder** in the app grid.
- **Files, when it closes:** however it closes, a Files tab showing the folder or a search closes with it, and a window
  with no other tab, so that no file's name stays on the screen. Other windows and tabs stay. A folder you went back out
  of can still be in that window's back button, by name.
- **Search:** GNOME's file index, which remembers the names of your files and the words in them, skips every folder
  named **Private**, wherever it is in your home, so this folder's files are never in it and the search never shows
  them. On a machine that had indexed the folder before, the index forgets it within a minute of the update.
- **A new password:** nothing is copied; only the password that opens the files changes.
- **Taking it away:** it never deletes files: move them out first.

### What it does not cover

- **Copies made elsewhere.** A program's own autosaves and backups (LibreOffice's backup folder, an editor's swap
  files), a download saved straight into Downloads, screenshots and the clipboard are outside it. When it closes, the
  thumbnails of its pictures and its files' places in the lists of recent files are taken away.
- **Files open when the screen locks.** It closes without waiting: a program with a file open in it can no longer read
  or save that file, and keeps what it had not saved in its window. A notification names the programs. Unlock, open the
  folder and save again.
- **Flatpak apps.** An app allowed your whole home sees the folder while it is open; the others reach its files only
  through the file chooser.
- **What the encrypted copy shows.** The copy in `~/.local/share/amethystora/private` shows how large each file is and
  how its folders are arranged, but no names and no contents.
- **Backups and snapshots** hold only that encrypted copy: [the backup](backups.md) never copies the open folder, and
  [the hourly snapshots](viruses.md#ransomware-protection) hold the same encrypted copy. Putting it back needs the
  password or the recovery key.
- **While it is open,** a program running as you can read it, or encrypt it like any other folder; the snapshots keep
  the copy from before.
