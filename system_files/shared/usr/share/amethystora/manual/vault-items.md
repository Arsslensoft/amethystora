# Passwords, codes and keys

What the Vault holds day to day: passwords, authenticator codes, backup codes and cryptographic keys. Certificates have
[a page of their own](vault-certificates.md).

## Passwords

**Passwords** keeps a title, username, address, password and note for each, and its last five passwords with their dates.
Search is as quick with a thousand as with ten.

### Making and copying a password

- **Make one.** A random password of 8 to 128 characters (20 by default, from the sets you pick), or a passphrase of words
  (six by default, from the Electronic Frontier Foundation's large word list). The window shows the real strength in bits of
  what it generated, and only a marked estimate for one you typed.
- **Reused passwords are marked.** Nothing checks a list of leaked passwords, since that would send something away.
- **Copy** marks the text as a secret, so [Clipboard](clipboard.md#the-clipboard) leaves it out of its history, and takes it
  off after 30 seconds if it is still yours.
- **A password shown** hides after 30 seconds and when the window loses focus, and the window is covered while the whole screen
  is shared.

`ame vault password` (`generate`, `add`, `new`, `list`, `get`, `remove`) in a terminal. A password is never an argument: `add`
asks for it, and `get` prints it after you approve, for a script.

## Authenticator codes

The sign-in codes that change every thirty seconds. **Authenticator** shows each in two halves (`287 082`), the seconds left,
and the next code when under five remain. The account's secret stays in the service, which hands back only the code.

![A code counting down](media/vault-code-countdown.webm)

- **Kinds.** Time-based (TOTP), counter-based (HOTP) and Steam Guard's five characters, with the usual algorithms, digits and
  periods. A counter moves only when you ask for the next code, never back.
- **A wrong clock gives wrong codes.** The window says when the clock is not synchronized.
- **Beside the password, a code is less of a second factor**: whoever opens the vault has both. Keep them in two vaults, or not
  here, if you want them apart.

`ame vault auth` in a terminal.

### Adding one

**Add** takes a code by **scanning its QR code** with the camera (refused while `ame security block-webcam` is on), from **a
window on the screen** (GNOME's screenshot portal, kept in memory only), from **a picture file**, or by **typing** the setup key,
the `otpauth://` link or each part. The QR code is read in the window, never in the service, which then reads the link strictly.
A setup key is at least 10 bytes, in the letters A to Z and digits 2 to 7.

### Showing a code to another app

**Show QR code** draws the `otpauth://` link as a QR code, with the key below in groups of four for an app that cannot scan. It
asks every time and starts blurred, saying that **anyone who sees it can make your codes**. It hides after 30 seconds and when
the window loses focus, is never written to a file, and is covered while the screen is shared.

### Moving between apps

- **From BackMyCodes.** **Move from BackMyCodes** opens its export files and reads its transfer QR codes in any order (each
  ticked off as read). A transfer is a series of codes with a password. The Vault writes the same format, accounts and backup
  codes together, so BackMyCodes can read it. A transfer with no password is read, with a warning, and never written.
- **Imports**, each shown before anything is kept, to merge or replace: Google Authenticator's migration QR codes, Aegis and
  2FAS (plain or encrypted), Bitwarden's unencrypted export, and a text file of links.
- **Export** writes an encrypted BackMyCodes file, then asks for its password again to prove the file opens. The window counts
  accounts added since the last export.
- Each account has a note, tags and a custom order. Removing one offers **Undo** for a few seconds.

### Service logos

**Off** until you turn it on, and asked once. It shows each account with the site's own icon. **That tells each site that
someone at this address has an account there**, so a row on *What this machine sends* names it once it is on.

- It asks only the account's own site for `/favicon.ico` and `/apple-touch-icon.png`, never an icon service that would learn
  the whole list, and sends only a host name, never a field that could hold a secret.
- A picture is untrusted: decoded in a sandbox, kept to 256 KiB, and stored sealed in the vault, since a folder of them would
  list your accounts.
- It waits on a metered connection, and **a hidden vault never fetches one.**

`ame vault logos on|off` in a terminal.

## Backup codes

The one-time codes a service gives you in case you lose your phone. Each service is an item with the account, a note and its
codes, and each code knows when it was used.

- **Saving them.** Paste them, or pick a text file or PDF. Numbering, bullets and blank lines are taken out, and you confirm the
  count first. Pictures are not read: there is no text recognition.
- **Using them.** **Next code** shows one with **Copy** and **Mark used** (with **Undo**). A used code stays, struck through,
  with its date.
- **A warning at two left**, with a link to the service's page for new ones.
- **Accounts with none.** An authenticator account counts as covered when a service of the same name keeps backup codes here,
  unless you marked it as offering none; the list names the rest.
- **Showing every code, or printing them,** asks every time. `ame vault codes` in a terminal.

## Keys

**Keys** makes cryptographic keys inside the service, from the computer's random source. The window only ever holds the public
half.

| Kind | Types | Default |
| --- | --- | --- |
| **SSH** | Ed25519, ECDSA P-256, RSA 3072 and 4096, or one you bring in | Ed25519 |
| **Signing** | Ed25519, ECDSA P-256 and P-384, RSA 3072 and 4096, ML-DSA-65 | Ed25519 |
| **Exchange** | X25519, ML-KEM-768 | X25519 |
| **Symmetric** | AES-256 | |

ML-DSA and ML-KEM (post-quantum) are offered only where this computer's OpenSSL has them.

### Public and private halves

- **The public half** is free to hand out, with no approval: as PEM, as the line `ssh` wants, and its SHA-256 fingerprint.
- **The private half** is exported only as PKCS#8, locked with a passphrase unless you say otherwise, asking every time, after
  saying that **from then on the key lives outside the vault**, where any program of yours can read the file. A key made with
  **Can be exported: no** never leaves; that cannot be changed later.
- **Bringing in an SSH key** (`ame vault key import`) asks for its passphrase and offers to take the file out of your home. Old
  copies in snapshots and backups stay ([Protected files](vault-secrets.md#protected-files)).

`ame vault key` in a terminal.
