# Logins

The ways to sign in besides your password, the switches that guard a login, and the screen lock that follows your phone.

## Passwords, codes and keys

**Vault** keeps your passwords, authenticator codes, backup codes, keys and certificates where the programs you run
cannot read them, and asks you before each use ([Vault](vault.md)). It is a security feature of its own, with its own
window, and no switch here.

## Security keys

A FIDO2 security key (YubiKey, Thetis, or a fingerprint model such as the YubiKey Bio) can sign you in, unlock the
screen and approve `sudo` and administrator prompts instead of your password. Your password keeps working whenever no
registered key is plugged in.

```bash
ame security key                  # the menu
ame security key add              # add a key
ame security key add-fingerprint  # add a fingerprint model
ame security key remove           # forget every key at once, without asking
```

For a fingerprint model choose **Add a fingerprint key**, so that the key checks your finger. The key needs a PIN, and
a fingerprint model an enrolled finger, first: set them in Firefox at `about:webauthn`, or with
`ykman fido fingerprints add` on a YubiKey Bio. Register a second key as a spare.

## Fingerprint login

On a machine with a fingerprint reader, a finger can sign you in, unlock the screen and approve `sudo` and
administrator prompts, and your password keeps working beside it.

```bash
ame security fingerprint          # the menu
ame security fingerprint add      # Add a finger: enrol one; enrol a second as a spare
ame security fingerprint test     # Test a finger: check that the reader knows it
ame security fingerprint remove   # Remove all fingers: forget every one
```

**Settings › System › Users** does the same under **Fingerprint Login**. Two things still ask for the password: the disk
passphrase at boot, and the keyring, which a sign-in by finger leaves locked until an app needs it. The readers that
work are listed at [fprint.freedesktop.org](https://fprint.freedesktop.org/supported-devices.html).

## A duress finger or password

A duress finger, or a duress password, is a second way to unlock the machine, for when someone makes you unlock it in
front of them. At the login screen and the lock screen it unlocks the machine like your usual finger or password, so
nothing looks different. At `sudo`, administrator prompts, the text consoles and over SSH it is refused, as a finger or
a password that did not match, so it never hands over administrator rights. Either way, an action you chose beforehand
runs as root, once until the next start.

Set it up in the Security app, **Settings**, **Under duress**, whose every step opens in a terminal, or:

```bash
ame security duress
```

### Setting it up

- **Set up a duress finger** needs an ordinary finger enrolled first, and a free place for one more on the reader. It
  enrols one more finger, has you touch the reader with it and with another finger to check that the reader tells them
  apart, and asks what its action should be.
- **Set up a duress password** asks for a second password, only yours and different from the one you log in with, and
  asks the same.
- The finger and the password **each have their own action**. Change either later with **Change the duress finger's
  action** or **Change the duress password's action**.
- An action is one of: an alert sent to an address of yours (an [ntfy.sh](https://ntfy.sh) topic only you know, for
  example), the disk's key taken out of the TPM so that the next start asks for the passphrase
  ([Unlock the disk with the TPM](disk-encryption.md#unlock-the-disk-with-the-tpm)), power off ten minutes later, or a
  script of your own, which it shows you before it keeps it. None of the examples erases anything, but the action is an
  ordinary root script and yours to write, so it can do whatever root can.
- **Test my duress finger** says which finger a touch matched and never runs the action.
- **Remove my duress finger** and **Remove my duress password** forget yours (removing the finger asks whether to take
  it off the reader too, or keep it as an ordinary finger). **Turn off for everyone** forgets every account's and both
  actions, keeping the fingers as ordinary ones.

### What it needs and what it cannot do

A duress finger needs a reader that compares a touch with every enrolled finger at once, which most readers built into
laptops do; the setup says so when yours does not. A duress password needs no reader.

Nothing about either reaches the journal or the Security report while it works, and the setup lives in a folder only
root can read. The security watcher checks each: when an action, or a duress finger's print, changes other than through
`ame security duress`, that action no longer runs, and the report and a notification say so under its own heading.
Enrolling the finger again, or **Remove all fingers** in `ame security fingerprint`, removes a duress finger too; set it
up again afterwards.

It cannot stop anyone asking for your usual password instead, help against a copy of the disk made before, or hide from
someone who already has root on this machine. In many places, destroying data while compelled by the authorities is an
offence of its own; what the action does is yours to decide.

## One-time codes for SSH

If you turn the SSH server on and log in to this machine with a password, a stolen password is enough to get in. With
one-time codes on, a password login over SSH also asks for a six-digit code from an authenticator app on your phone,
such as Aegis or FreeOTP. A login with an SSH key works as before, and nothing changes at the machine itself.

Set up your own codes first, then turn them on:

```bash
ame security ssh-codes setup
ame security ssh-codes on
```

- `setup` shows a picture to scan with the app, then asks for the code it shows. Keep the emergency scratch codes it
  prints somewhere safe and away from this machine: each one logs in once without the phone.
- `on` refuses until the account running it has codes or an SSH key in `~/.ssh/authorized_keys`, so that it cannot lock
  you out, and keeps the change only if the SSH server accepts it.
- Each account that logs in over SSH with a password sets up its own; an account without codes can then log in over SSH
  only with a key.
- `ame security ssh-codes off` turns them off and keeps everyone's codes.

With a FIDO2 security key there is another way that needs nothing on this machine: make an SSH key that lives on it with
`ssh-keygen -t ed25519-sk` on the computer you connect from, and it asks for a touch of the key at each login.

## Parental controls

For a child's account, make it a standard account, not an administrator, in **Settings › System › Users**, then open
**Parental Controls** from the app grid, or from that account's page in Settings. It decides which installed apps the
account may open, whether it may install apps and up to which age rating, and, from GNOME 50, how long a day it may use
the computer and from what time in the evening it may not.

When the time is up, the screen locks, and that account cannot unlock it until its time comes round again. The text
consoles (`Ctrl+Alt+F3`) and SSH have no lock screen, so there the account cannot log in outside its hours, and a
session there is closed when its time runs out, with anything unsaved in it. There the daily limit counts each session
afresh, and no session runs past midnight while a limit is set.

## Guest login

For a friend who needs a browser for ten minutes: the login screen lists **Guest**, who logs in without a password, into
a home folder kept in memory. Everything a guest does is gone when they log out: their files, their browser's history
and passwords, the apps they installed for themselves. They see none of your files, since every account's home folder
is closed to the others. It is **off**:

```bash
ame security guest-login on
```

or **Guest login** on Security's **Settings** page, under Logins. It says what it does and asks first.

- Guest is a standard account: it cannot install apps for the machine or change its settings. Parental controls can be
  set on it like on any account.
- Only the login screen lets Guest in without a password. The text consoles, SSH, `sudo` and `su` refuse it.
- What a guest does online is done from your network and your address.
- Turned off, a guest still logged in is logged out, and the account is deleted.
- Security's report lists it while it is on, and warns if part of it is missing.

## Walk-away lock

The screen locks when your paired phone has left for a minute (it left the home network, or the computer did) and
nobody has touched the computer for a minute: a short card says *Your phone left · locking*, then GNOME's own lock comes
down. It is off as the image ships, and in no profile.

![The phone leaves, the screen locks](media/walk-away-lock.webm)

```bash
ame security walk-away on    # or Security › Settings › Logins
ame security walk-away off
```

- It never unlocks anything, and it locks once each time the phone leaves: after you unlock, it waits for the phone to
  come back and leave again.
- It needs the [phone link](phone.md) on and the phone paired; turning the phone link off is not the phone leaving.
- It does not lock while something keeps the screen awake (a video playing, a presentation). A phone that puts its Wi-Fi
  to sleep only makes it lock a little sooner.
