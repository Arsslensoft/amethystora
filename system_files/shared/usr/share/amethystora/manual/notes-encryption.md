# Locking your notes

How [Notes](notes.md) encrypts what you write, the passkeys that open it, and how to keep a copy.

## Encryption

With encryption on, your notes, tasks and attachments are written to disk encrypted with AES-256-GCM, under a random
key that only your passphrase, or a [passkey](#passkeys) you add, can open. The passphrase is stretched with scrypt,
which costs 128 MiB of memory for every guess, so trying passphrases one after another is slow.

> **Nobody can recover a forgotten passphrase.** Not Amethystora, not an administrator, not anyone.
> Keep it somewhere safe, and keep a [backup](#backing-up).

### Against quantum computers

It stays secure against quantum computers. What they break is public-key cryptography (RSA, elliptic curves), and none
is used here. Against a cipher like AES the best they can do is halve the key length, which leaves AES-256 at 128-bit
strength: why NIST and the NSA's CNSA 2.0 keep AES-256 for the years after quantum computers.

### When your notes lock

Your notes lock:

- after 10 minutes without using Notes, or 5, 15 or 30 minutes, an hour, or only on the other two
  (**Settings**, **Lock when away**),
- when the screen locks, the computer sleeps, or the window closes,
- when you press **Ctrl+L** or **Lock** in the sidebar.

### Changing the passphrase

A passphrase needs at least 8 characters, typed twice. **Settings** changes it, and turns encryption off or on again.
Changing the passphrase changes the key too, unless you turn **Change the key as well** off: everything is encrypted
anew, so that an older copy of the notes folder, from a snapshot or a backup, cannot open what you write from now on
with the old passphrase. That older copy itself still opens with it. Passkeys have to be added again after a new key.

While your notes are encrypted, Notes reads only encrypted files: notes or an attachment put in the folder unencrypted
are not read.

### Passkeys

A FIDO2 security key (YubiKey, Thetis, or a fingerprint model such as the YubiKey Bio) can open your
notes, so that you need not type the passphrase each time. The passphrase keeps working, and it is
what opens your notes if the key is lost: a passkey is a second way in, never the only one.

#### Adding a passkey

To add one, plug the key in, open **Settings** and choose **Add a passkey**. Enter your passphrase and
the PIN of the key, or leave the PIN empty for a key that reads your fingerprint. The key blinks twice;
touch it each time. From then on the lock screen offers **Open with a passkey**, which asks for the
key's PIN, or your fingerprint, and a touch. Have only that key plugged in.

#### What to know about passkeys

- **The key needs a PIN**, and a fingerprint model an enrolled finger. Set them in Firefox at
  `about:webauthn`. A touch alone never opens your notes: someone who finds the key cannot use it
  without the PIN or your finger, and the key blocks its PIN after eight wrong tries.
- **The key has to support FIDO2's `credProtect`**, with which Notes tells it to refuse the passkey
  without the PIN or a fingerprint. Notes says so when a key does not. A passkey added before Notes
  asked for it is marked in **Settings**: remove it and add it again.
- **Add a second key as a spare**, one at a time. Each is listed in **Settings**, where **Remove**
  stops it opening your notes.
- **If a key is lost**, remove it, then change the passphrase with **Change the key as well** on.
  Someone who holds the key, knows its PIN and has an older copy of your notes folder could otherwise
  open your notes with it. Add the keys you still have again afterwards.
- A [backup](#backing-up) always asks for the passphrase: passkeys are not part of it.

The key keeps nothing of your notes. It holds a secret that never leaves it, and with the PIN or
fingerprint checked it computes a value (HMAC-SHA256, FIDO2's `hmac-secret`) under which a second
copy of your notes' key is encrypted. What is on disk is still encrypted with AES-256 only.

## Backing up

Since nothing is synced, keep a copy somewhere else. In **Settings**:

- **Back up to a file** writes one `.amnotes` file with everything in it, attachments included,
  encrypted if your notes are. It opens with the passphrase you had when you made it, on any computer:
  choose it under **Import**, which adds what is in it to the notes already there.
- **Export as Markdown** writes every note as a Markdown file in a folder per notebook, its attachments
  beside them, and the inbox and every project as a checklist. These files are **not** encrypted.

Your notes themselves are in `~/.local/share/amethystora-notes`.
