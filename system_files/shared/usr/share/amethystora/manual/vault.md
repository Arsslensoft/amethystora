# Vault

**Vault** in the app grid keeps your passwords, authenticator codes, backup codes, keys, certificates and secret files away
from the programs on your computer. It takes the place of a password manager, an authenticator app and a folder of key files.

Nothing leaves this computer unless you connect a server of your own ([Sync](vault-sync.md#sync)) or turn on
[service logos](vault-items.md#service-logos). Both are off until you turn them on.

## What it is, and what it protects you from

A program running as you can normally read everything you can. So the Vault keeps nothing in your home folder and does not
run as you: a small service, under an account of its own, with no network, holds it. It starts when something asks and goes
when the Vault is closed and idle. Your files, your programs and a Flatpak with access to your home cannot read its files or
its memory.

### Every use is approved

A program running as you can still *ask* the service. So every use of a secret goes through a prompt the program cannot
answer for itself: GNOME's own, answered with your fingerprint or your account's password.

![Approve, then fill](media/vault-approve-fill.webm)

> A program calling itself ssh asks to sign with "work server" from Amethystora Vault.

The name is what the program calls itself. It proves nothing, and the Vault never uses it to skip a question. If something you
did not start asks, say no.

- **The prompt is yours.** Over SSH, or from a session not at the screen, the answer is always no. Say no three times in a
  minute to one program and it is not asked again for a minute.
- **How often an item asks** is its own setting ([How each item asks](#how-each-item-asks)).
- **The AI agent never gets in.** Its tools refuse everything of the Vault and the commands that reach it
  ([AI agent](ai-agent.md)). A program that is not the agent, but runs as you, is what the approvals are for.

### How it is sealed

Each item is one sealed file, encrypted with AES-256-GCM under one random key, which is itself sealed once for each way of
opening. Without one of those ways, **nobody can open it: not Amethystora, not an administrator**
([The recovery key](#the-recovery-key)).

## Setting it up

Open **Vault**, or run `ame vault setup`, and it asks how it should open. Setup ends with your **recovery key**, which you type
back before it finishes. A machine where nobody sets up a vault has nothing running and nothing kept: only a socket, until
something asks.

## How it opens

Add more than one way, and take one away again, with `ame vault methods`.

| Way | You do | It stops a program that asks | It needs |
| --- | --- | --- | --- |
| **Vault password** | Type it | Yes | Nothing |
| **Security key** | Touch it, with its PIN or fingerprint | Yes | A FIDO2 security key |
| **Fingerprint or account password** | Touch the reader, or type your account's password, when GNOME asks | Yes | A TPM, or see below |
| **No prompt** | Nothing | **No** | A TPM, and your say-so |
| **Recovery key** | Type it | Yes | Always made at setup |

- **Password and security key** run on your side: the service cannot open the vault without them, even as root. The password
  has at least 8 characters and is stretched with scrypt (128 MiB of memory per guess, as the [Notes](notes.md) passphrase).
- **A security key** works as for Notes ([Passkeys](notes-encryption.md#passkeys)): FIDO2 with `credProtect`, always with its
  PIN or a fingerprint, never a touch alone.
- **Fingerprint or account password** seals the key to this computer's TPM, opened when GNOME's prompt is answered. It is the
  default where there is a TPM. Without one it is still offered, and says so: **then only the disk's encryption protects the
  key while the computer is off.** Changing the vault password changes nothing here.
- **No prompt** asks nothing at all. Setup has you type what that gives up: it stops a program that reads your files, a copied
  backup and a Flatpak with access to your home, **and it does not stop a program of your account that asks**, which then gets
  whatever the open vault holds. Never offered without a TPM.
- **The recovery key** is the way back if every other way is lost.

### When it closes

The vault wipes its key from memory:

- when you lock the screen, log out, or the computer sleeps (the sleep waits for the wipe);
- after 15 minutes without use (`ame vault methods idle MINUTES`, or in the window);
- on **Lock now** or `ame vault lock`;
- when the service stops, however it stops.

Using an item keeps it open longer, and so does the Authenticator page while codes show; syncing never does. A vault that failed
to close counts as closed: it fails closed.

### How each item asks

| Setting | The vault asks | The default for |
| --- | --- | --- |
| **Ask every time** | At every use | Private keys of keys and certificates |
| **While open** | The first time, then not until the vault closes (15 minutes for an SSH key) | Passwords, codes and SSH keys |
| **Never ask** | Not at all while the vault is open | Nothing |

**Never ask** is chosen in the window, after it says what you give up: any program of your account can then take that item
unasked. The list of names needs an open vault and no approval. Taking a secret *out* (a key, a PFX file, every backup code, a
QR code) always asks every time, whatever the setting.

### The recovery key

Setup shows a 256-bit key in groups of letters and numbers. **Write it down or print it, and keep it away from this computer.**
You type it back before setup finishes.

- It opens the vault if you lose the password, security key or TPM; then make a new way in under **Your vaults**.
- **Nobody can make you a new one, and nobody can recover a forgotten password**: not Amethystora, not an administrator.
- It opens a [backup](vault-sync.md#backing-up-and-moving) on a computer that has never seen this vault.
- One recovery key per vault, never split into shares.

## Several vaults, and hidden ones

An account can have several vaults (Work, Personal), each with its own password, recovery key and items: `ame vault new` or
**Make a vault**. The first is the one the window opens to, and the only one with your certificates.

A **hidden** vault is listed nowhere: not in the window, `ame vault status`, the journal or the report. Type its password on
the closed screen to open it. Only its password opens it, never a fingerprint, security key or no prompt, since each of those
would have to say which vault to offer.

### What a hidden vault does not do

- It does not hide from **someone who reads this computer's storage as an administrator**, who can count the folders. A sealed
  backup holds every vault, and its size can say as much.
- It does not hide from a program, which can type a password too. It hides a vault from someone looking at the window.
- It never fetches a service logo and is never offered sync, since both would show.
- Removing it needs it open. `ame vault forget` removes **every** vault of your account and always says "including any you
  cannot see", so the question never tells whether there is one.

Locking closes them all. The SSH agent, `ame vault run` and the AI agent see only what is open.

## What it does not do

- **While a vault is open with *While open*, a program of your account can ask without a prompt.** Use short times and *Ask
  every time*.
- **A secret handed to a program is that program's.** The Vault cannot take it back.
- **A program can draw a fake prompt** and ask for the password. A security key or fingerprint resists that; a typed password
  does not.
- **Root can reach everything** not sealed under a password, security key or recovery key, and those too while the vault is open.
- **A whole older copy of the vault put back is not noticed.** One item put back is; a whole folder restored from a disk backup
  opens as it was.
- **A program that keeps asking keeps the vault open** and makes prompts appear. Three noes a minute quiet it for a minute, and
  a program can change its name.
- **No certificate the world trusts**, no revocation, no timestamp of what you signed (a signature may be called expired after
  its certificate). See [Certificates and signing](vault-certificates.md).
- **No sharing**, no organizations' collections, no passkeys for websites, no import from other password managers beyond what
  sync brings in and an existing SSH key; no backup of the machine's root, no hosted authority.

## Commands

A command takes its words as arguments and never a secret. `ame vault` lists them; those that ask open a terminal from the
window.

| Command | Does |
| --- | --- |
| `ame vault setup`, `open`, `lock`, `status`, `vaults` | Make, open, close and list vaults |
| `ame vault methods` | Ways it opens: `add`, `remove`, `idle MINUTES`, `background on|off` |
| `ame vault change-password`, `new`, `rename`, `remove-this`, `forget` | Passwords and vaults |
| `ame vault password`, `codes`, `auth`, `key`, `cert` | Passwords, backup codes, authenticator codes, keys, certificates |
| `ame vault pdf sign FILE`, `root`, `root rotate` | Signing and the machine's root |
| `ame vault ssh on|off|status`, `helpers`, `get`, `run`, `protect`, `protected`, `unprotect` | Using a secret without handing it over |
| `ame vault sync`, `logos`, `tls` | Sync, service logos, trusting the local CA |
| `ame vault backup`, `restore FILE` | The sealed backup |
