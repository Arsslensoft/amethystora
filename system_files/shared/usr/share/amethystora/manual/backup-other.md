# Restoring, and other computers

## Starting from your backup

A stolen laptop should be an afternoon, not a lost month. The recovery key opens the backup on any computer, with
nothing else.

### The recovery key

`ame backup recovery-key` adds a second key to your backup's repository (it needs the credentials the backup already
runs on), shows it once as five groups of five letters and digits to write on paper, and keeps it nowhere: not on this
computer, not in your [Vault](vault.md). Losing the laptop, and the Vault that held the backup's own password, then
costs you nothing. Anyone who has the key and can reach the backup can read it, so keep the paper apart from the
computer.

### On a new computer

The first look's **Have a backup?** (or `ame backup restore-from`, or **Start from your backup** in
[Control](control.md)) asks where the backup is: a backup disk it finds when you plug it in, an address, or your other
computer (below). It asks for the recovery key (typed, held in a private file only until it is finished), says what the
newest backup holds ("Your files, from Tuesday; your setup, with its apps") and lets you choose each step:

- **Your files** come back with Backups' own putting back: what is missing is added, a file that is here and differs is
  kept beside the old one, and nothing is replaced or deleted. Only for a backup of the same home folder (the same
  account name as before).
- **Your apps and setup** is `ame restore-setup`, which asks again before each part.
- **Your Vault** comes back from the sealed backup that was with your files, asking for the Vault's own recovery key.

Starting from a backup never changes how this computer backs up: nothing is set up to back up from it until you run
`ame backup`.

## Back up to your other computer

Two computers at home can look after each other: no disk to remember, no cloud, no subscription. One keeps the backup
(the store); the other is backed up to it, nightly.

### The store

On the computer that will **keep** the backup, turn on **Be another computer's backup store** in Security, which opens a
pairing port and a range of ports in the zone of the networks you named home and nowhere else, then:

```bash
ame security backup-store on
ame backup store on
ame backup store pair
```

The store's own key is **sealed in its Vault** (under the background key, so the store can start with the Vault
closed), and is on disk only for the moment the servers start.

### Pairing

On the computer to back up, `ame backup other-computer setup NAME` pairs with it: **both screens show six digits, and
you say whether they are the same**. A stranger in the middle would make the two screens differ, and nothing is kept on
either side then.

- Use the name the store's certificate carries (its host name, or that name with `.local`, or an address it had when it
  was set up): setup says so before it asks for anything if the name you typed is not one of them.
- Your Vault must be set up: the backup's password and this computer's key for the store are **sealed in it** under its
  background key, which works with the vault closed so that a night's backup needs no one (the Vault's setup says what
  that gives up).
- The command also makes a [recovery key](#the-recovery-key).

### The nightly run

Each hour that both computers are at home, the other answers, and the last run is more than twenty hours old, the same
backup as `ame backup` runs there too. If the Vault is closed and has no background key, the night waits and a notice
says so once a day.

```bash
ame backup other-computer now     # run it at once
ame backup other-computer status  # where it stands
ame backup other-computer off|on  # the nightly run
ame backup other-computer forget  # take this computer's settings and keys away
```

### What the store can and cannot do

- It holds **only encrypted data**, because restic encrypts before it sends.
- It runs `rclone serve restic --append-only` over TLS 1.3, **one server, one port and one folder for each computer you
  paired**, and each server accepts that computer's certificate and no other: no password crosses the network, and one
  paired computer can neither read nor write another's backups.
- **A computer can add and read its own backups but can never delete from them**, so what was backed up before it was
  taken over is still there afterwards.
- The data is in its owner's home (`~/.local/share/amethystora/backup-store/data`), which the store's own backup leaves
  out. It fills the store's disk until someone thins it: **nothing thins it by itself**, because that needs the
  repository's password, which the store never has. At the store, type the recovery key for:

```bash
restic -r ~/.local/share/amethystora/backup-store/data/FOLDER forget --keep-daily 14 --keep-weekly 8 --prune
```

`ame backup store clients` lists each computer it accepts, with its fingerprint; its folder is the first sixteen
characters.

### Replacing or forgetting a computer

- `ame backup store forget FINGERPRINT` stops accepting one. A computer paired only to restore stays in the store's list
  until you forget it.
- **A new computer for a lost one** is paired as a replacement, `ame backup store pair --replace FINGERPRINT`: it takes
  over that computer's backups, port and folder, the old certificate is no longer accepted, and
  `ame backup restore-from` on it brings everything back with the recovery key.
- Each `ame backup recovery-key` adds another key, and on an append-only store none can ever be taken away, so keep the
  paper safe.

Nothing changes for the backup in `ame backup` and the Backups window, which go on as before; they do not show the other
computer's backup yet.

## Your backup disk

When your backup goes to a USB drive (a repository on a disk, as `ame backup setup` set it up), plugging that disk in
asks **Back up now?**, with when the last backup was.

- **Back up now** starts the same backup as Backups' button and says when it is done; the disk is then put away and you
  are told it is **Safe to unplug**. If a window still has a file open on it, you are told to put it away in Files
  first.
- **Not now** leaves it.
- The disk is known only by the folder your backup was set up in; its password and settings are not read.
- `ame desktop backup-disk off` (or Control's **Apps** page) asks nothing when it is plugged in.
