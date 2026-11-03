# Vault

**Vault** in the app grid keeps the secrets you use every day in one place, away from the programs on your
computer: your passwords, the codes of your authenticator accounts, your backup codes, cryptographic keys, certificates
for signing PDFs and programs, and files such as an SSH key or a login you want kept out of your home folder. It
takes the place of a password manager, an authenticator app and a folder of key files.

Nothing leaves this computer unless you connect a server of your own (see [Sync](#sync)) or turn on the service logos
(see [Service logos](#service-logos)). Both are off until you turn them on.

## What it is, and what it protects you from

A program running as you can normally read everything you can. So the Vault does not keep its files in your home
folder, and it does not run as you. A small service, started only when something asks and gone again when the Vault
is closed and idle, runs under an account of its own, with no network and no right to do anything it does not need.
Your files, your programs and a Flatpak with access to your home cannot read its files or its memory.

That is the first half. The second half is that **a program running as you can still ask the service**, and so every
use of a secret goes through an approval that the program asking cannot answer for itself: GNOME's own prompt, which
you answer with your fingerprint or your account's password. The prompt says what is asking and for what:

> A program calling itself ssh asks to sign with "work server" from Amethystora Vault.

The name is what the program calls itself, which is all the computer can tell. It proves nothing, and the Vault never
uses it to skip a question. If something you did not start asks, say no.

- **The prompt is yours to answer.** Over SSH, or from a session that is not at the screen, the answer is always no.
  If you say no three times in a minute to one program, it is not asked again for a minute.
- **How often an item asks** is its own setting (see [How each item asks](#how-each-item-asks)).
- **The AI agent never gets in.** Its tools refuse everything of the Vault, and the commands that reach it, and
  nothing here is offered to it (see [AI agent](ai-agent.md)). A program that is not the agent, but runs as you, is
  the case the approvals are for.

Everything the Vault holds is one sealed file for each item, encrypted with AES-256-GCM under one random key, which is itself
sealed once for each way of opening. Without one of those ways in, **nobody can open it: not Amethystora, not an administrator**
(see [The recovery key](#the-recovery-key)).

## Setting it up

Open **Vault** and it asks how it should open. `ame vault setup` does the same in a terminal. Setting up ends with
your **recovery key**, which you have to type back before it is finished (see [The recovery key](#the-recovery-key)).
A machine nobody sets a vault up on has nothing running and nothing kept: the Vault costs only a socket until
something asks.

## How it opens

You choose, and can add more than one way and take one away again (`ame vault methods`).

| Way | You do | It stops a program that asks | It needs |
| --- | --- | --- | --- |
| **Vault password** | Type it | Yes | Nothing |
| **Security key** | Touch it, with its PIN or fingerprint | Yes | A FIDO2 security key |
| **Fingerprint or account password** | Touch the reader, or type your account's password, when GNOME asks | Yes | A TPM, or see below |
| **No prompt** | Nothing | **No** | A TPM, and your say-so |
| **Recovery key** | Type it | Yes | Always made at setup |

- **The password and the security key** run on your side. The service cannot open the vault, even as root, without
  them. The password is stretched with scrypt, which costs 128 MiB of memory for every guess, as the [Notes](notes.md)
  passphrase is. A password has at least 8 characters.
- **A security key** works as it does for Notes (see [Passkeys](notes.md#passkeys)): a FIDO2 key with `credProtect`,
  and always with its PIN or a fingerprint, never a touch alone.
- **Fingerprint or account password** seals the vault's key to this computer's TPM, so that it opens when GNOME's prompt
  is answered. It is the default where a TPM is there. A computer without a TPM still offers it,
  and says so: **then only the disk's encryption protects the key while the computer is off.** It never involves the
  password of the vault, so changing that changes nothing here.
- **No prompt** opens the vault with nothing asked at all. Setup says what that gives up in so many words and asks you to
  type them: it stops a program that reads your files, a copied backup and a Flatpak with access to your home, **and
  it does not stop a program of your account that asks**. That program gets whatever the vault holds, as long as it is open.
  It is not offered without a TPM.
- **The recovery key** is not for every day. It is the way back if every other way is lost.

### When it closes

The vault wipes its key from memory:

- when you lock the screen, when the computer goes to sleep (the sleep waits for the wipe) and when you log out;
- after 15 minutes without use, which you can change (`ame vault methods idle MINUTES`, or in the window);
- when you press **Lock now**, or run `ame vault lock`;
- when the service stops, however it stops.

Using an item keeps the vault open for another while, and so does the Authenticator page while its codes are showing; syncing
never does. A vault that failed to close counts as closed: whatever goes wrong, it fails closed.

### How each item asks

Every item has its own setting for how often it asks, and the window shows it on the item:

| Setting | The vault asks | The default for |
| --- | --- | --- |
| **Ask every time** | At every use | Private keys of keys and certificates |
| **While open** | The first time, then not again until the vault closes (15 minutes for an SSH key) | Passwords, codes and SSH keys |
| **Never ask** | Not at all, for as long as the vault is open | Nothing |

**Never ask** is chosen in the window, after saying what it gives up: any program of your account
can then take that item without anyone being asked. Seeing the list of names needs an open vault and no approval.
Taking a secret out of the vault (exporting a key, a PFX file, every backup code, a QR code) always asks every time, whatever
the setting says.

### The recovery key

At setup the Vault shows a key of 256 bits in groups of letters and numbers. **Write it down or print it, and keep it away from
this computer.** Setup does not finish until you have typed it back, so that you know it is right.

- It opens the vault if you lose the password, the security key or the computer's TPM. After opening with it, make a
  new way in under **Your vaults**.
- It is the only way back. **Nobody can make you a new one, and nobody can recover a forgotten password.** Not Amethystora,
  not an administrator.
- It is also what opens a [backup](#backing-up-and-moving) on a computer that has never seen this vault.
- There is one recovery key for each vault, and no splitting it into shares.

## Several vaults, and hidden ones

An account can have more than one vault, such as Work and Personal, each with its own password, recovery key and items.
`ame vault new` and **Make a vault** in the window make another. The first vault is the one the window opens to, and the only
one that holds your certificates; the rest hold passwords, codes and the like.

A vault can be **hidden**. A hidden vault is not listed anywhere: not in the window, not by `ame vault status`, not in the
journal, not in the report. You open it by typing its password on the closed screen. It never opens with a fingerprint,
a security key or without a prompt, only with its password, because each of those would have to say which vault to offer.

What a hidden vault does **not** do:

- It does not hide anything from **someone who can read this computer's storage as an administrator**, who can see that
  there are folders and count them. A sealed backup holds every vault, hidden or not, and its size can say as much.
- It does not hide from a program asking, which can type a password too. It keeps a vault from someone looking at the
  window, not from someone who already has the password.
- Nothing but its own password opens it: not a fingerprint, not a security key, not a feature of the computer.
- A hidden vault never fetches a service logo and is never offered sync, since both would show on the network or in a list.
- Removing it needs it open, and `ame vault forget` removes **every** vault of your account, always saying "including any you
  cannot see", so that the question never tells whether there is one.

Locking closes them all. The SSH agent, `ame vault run` and the AI agent see only what is open.

## Passwords

**Passwords** keeps a title, username, address, password and a note for each, and the last five passwords it had, with their
dates. Search is as quick with a thousand of them as with ten.

- **Make one.** The generator makes a random password of 8 to 128 characters (20 by default, with the sets you pick), or
  a passphrase of words (six by default, from the Electronic Frontier Foundation's large word list). The window shows
  how many bits of strength it really has for what was generated, and only an estimate, marked as one, for a password you
  typed.
- **Reused passwords are marked** where two items share one. Nothing checks a list of leaked passwords, since that
  would send something away.
- **Copy** puts the text on the clipboard marked as a secret, so that [Clipboard](desktop.md#the-clipboard) leaves it out of its
  history, and takes it off again after 30 seconds if it is still yours.
- **A password shown** hides again after 30 seconds and when the window loses focus, and the Vault's window is covered while
  the whole screen is shared.

In a terminal: `ame vault password` (`generate`, `add`, `new`, `list`, `get`, `remove`). A password is never an argument:
`add` asks for it, and `get` prints it after you approve, for a script.

## Authenticator codes

The codes of the sign-in apps, the ones that change every thirty seconds. **Authenticator** shows each as two halves
(`287 082`), the seconds left, and the next code when under five are. Your account's secret stays in the service, which
works out the code and hands back only the code.

- **Kinds.** Time-based (TOTP), counter-based (HOTP) and Steam Guard's five characters, with any of the usual algorithms,
  digits and periods. A counter-based code moves its counter only when you ask for the next one, and never back.
- **A wrong clock gives wrong codes.** The window says so when the computer's clock is not synchronized.
- **A code is a second factor, and kept beside the password in one vault it is less of one**: whoever opens the vault has
  both. If you want them apart, keep them in two vaults, or not here.

### Adding one

**Add** takes a code in any of these ways: by **scanning its QR code** with the camera (refused while `ame security
block-webcam` is on), by taking it from **a window on the screen** (through GNOME's screenshot portal, kept in memory only),
from **a picture file**, or by **typing** the setup key or the `otpauth://` link, or each part by itself. The QR code is read in
the window, never in the service, and the service then reads the link strictly. A setup key has to be at least 10 bytes
written in the letters A to Z and the digits 2 to 7.

### Showing a code to another app

**Show QR code** draws the same `otpauth://` link as a QR code, so that you can scan it into another app, with the key below it
in groups of four for an app that cannot scan. It asks every time, and starts blurred, saying that **anyone who sees it
can make your codes**. It hides after 30 seconds and when the window loses focus, is never written to a file, and is covered
while the screen is shared.

### Moving between apps

- **From BackMyCodes.** **Move from BackMyCodes** opens its export files, and reads its transfer QR codes in any order (each is
  ticked off as it is read). A transfer is a series of codes, with a password. The Vault writes the same, so that BackMyCodes
  can read what it writes, with the accounts and the backup codes together. A transfer with no password is read, with a
  warning, and never written.
- **Imports**, each shown before anything is kept, to merge with what is there or replace it: Google Authenticator's
  migration QR codes, Aegis and 2FAS (plain or encrypted), Bitwarden's unencrypted export, and a text file of links.
- **Export** writes an encrypted file in the BackMyCodes format, and asks for its password again afterwards to prove that the
  file opens. The window counts the accounts added since the last export, as a reminder.
- Each account has a note, tags, and a custom order. Removing one offers **Undo** for a few seconds.

### Service logos

Off until you turn it on, and asked once. **Service logos** shows each account with the site's own icon in place of a letter on a
colour. **That tells each site that someone at this address has an account there**, which is why it is off, and why a
row on *What this machine sends* names it once it is on.

- It asks only the account's own site for `/favicon.ico` and `/apple-touch-icon.png`, never an icon service that would learn
  the whole list at once, and only a host name goes out, never a field that could hold a secret.
- A picture from anywhere is untrusted: it is decoded in a sandbox, kept to 256 KiB, and kept sealed in the vault, since a
  folder of them would be a list of your accounts.
- It waits on a metered connection, and **a hidden vault never fetches one.**

`ame vault logos on|off` does the same in a terminal.

## Backup codes

**Backup codes** are the one-time codes a service gives you in case you lose your phone. Each service is an item with the
account, a note and its codes, and each code knows when it was used.

- **Saving them.** Paste them, or pick a text file or a PDF. Numbering, bullets and blank lines are taken out, and you see
  the count and confirm before anything is kept. A picture is not read: there is no text recognition in the image.
- **Using them.** **Next code** shows one and offers **Copy** and **Mark used**, with **Undo**. A used code stays in the list,
  struck through, with its date.
- **A warning at two left**, in the window, with a link to the service's page for new ones.
- **Which of your accounts have none.** An authenticator account counts as covered when a service of the same name keeps backup
  codes here, unless you marked it as offering none, and the list names the rest.
- **Showing every code, or printing them,** asks every time. In a terminal: `ame vault codes`.

## Keys

**Keys** makes cryptographic keys inside the service, from the computer's random source. The window only ever holds the public
half.

| Kind | Types | Default |
| --- | --- | --- |
| **SSH** | Ed25519, ECDSA P-256, RSA 3072 and 4096, or one you bring in | Ed25519 |
| **Signing** | Ed25519, ECDSA P-256 and P-384, RSA 3072 and 4096, ML-DSA-65 | Ed25519 |
| **Exchange** | X25519, ML-KEM-768 | X25519 |
| **Symmetric** | AES-256 | |

ML-DSA and ML-KEM, the post-quantum ones, are offered only where this computer's OpenSSL has them.

- **The public half** is free to hand out: as PEM, as the line `ssh` wants, and with its SHA-256 fingerprint. No approval.
- **The private half** is exported only as PKCS#8, locked with a passphrase unless you say otherwise, asking every time, and after
  saying that **from then on the key lives outside the vault**, where any program of yours can read the file. A key made with
  **Can be exported: no** can never leave. That is chosen when it is made and cannot be changed.
- **Bringing in an existing SSH key** (`ame vault key import`) asks for its passphrase, and offers to take the file out of your
  home. Old copies of it, in snapshots and backups, stay (see [Protected files](#protected-files)).

`ame vault key` does it in a terminal.

## Certificates and signing

A certificate says who signed something. **Certificates** makes them for signing PDFs and programs, for mail, for logging in to
a service of your own, for naming this computer, and for local websites.

### Who vouches

Every computer makes its own **Amethystora OS Root CA**, the first time anything needs it, and keeps it where only the
service can reach it. It never leaves the computer and is in no backup. Under it, each account that makes a certificate
gets its own **Code Signing CA** with the account's name in it, and only that signs your certificates.

```text
Amethystora OS Root CA          one for each computer, never leaves it
  +- <you> Code Signing CA      one for each account on it
       +- a PDF certificate     signs documents
       +- a code certificate    signs programs
```

The certificates say `OU=Amethystora Vault` in the subject, and the root says which computer it is by a number from its key
(`Machine 3FA107C2`), never the name of the computer, since signed files travel and a name would go with them. Your CA comes at
your first certificate, and only when you are at the screen: a session over SSH gets none, and neither does a guest account.

> **Only computers that trust this machine's root, or your CA, accept these. Other computers call the signer unknown until
> then.** The Vault says so beside every certificate and before every PDF it signs.

This is not the kind of certificate a public authority gives out. No public authority issues a signing certificate for a key
that lives in software, and Let's Encrypt does not issue them at all. A certificate from here proves that this computer's
Vault signed, to whoever chose to trust it. For files anyone in the world should accept, use a certificate from a
signing service, with its key on a card or in the service's own hardware.

### Making a certificate

**Make a certificate** asks which kind. The window shows how the subject will read before it is made.

| Kind | For | Lasts |
| --- | --- | --- |
| **PDF** | Signing documents | 460 days |
| **Code** | Signing programs (Windows programs, Java archives) | 460 days |
| **Mail** (S/MIME) | A mail program to sign and encrypt with. It needs your address | 460 days |
| **Local TLS** | A website or a device on your own network | 397 days |
| **Logging in to a service** | A client certificate for a VPN, Wi-Fi or a server of your own | 397 days |
| **This computer's identity** | Naming this computer. Needs an administrator | 397 days |

- A certificate never outlasts its CA. Your CA lasts five years and the root twenty.
- **A signer's name that looks like a host name** (`j.smith`) is refused, because other programs would refuse it. `J. Smith` is fine.
- A signing certificate holds a key of RSA 3072 by default, or RSA 4096, ECDSA P-256 or P-384. The key is made by the service
  and signs inside it.
- **Renew** makes a new certificate under the same name and marks the old one, which stays until you remove it. The window warns
  from 30 days before a certificate ends, whenever the vault is open.

### Signing a PDF

`ame vault pdf sign FILE` (or **Sign a PDF** in the window) signs a copy of the file, named `FILE (signed).pdf` beside it, and
never over anything. It asks first, once for each PDF, and the key never leaves the vault: the file's fingerprint goes in, and a
signature comes out.

What a signed PDF tells whoever opens it: **the signer's name, your name in the CA, and a number that identifies this computer.**
That is what lets someone check it, and it also means two documents can be seen to come from one computer.

A signature does not outlast its certificate unless it carries a timestamp from a time server, and a timestamp means sending a
fingerprint of the document to someone else's server. This version does not: **a reader may call the signature expired once the
certificate is**, 460 days on.

### Signing programs

**Export for signing (PFX)** gives a PKCS#12 file, locked with a password you type, which `osslsigncode` (part of this image),
`signtool` and `jarsigner` take. The key then lives outside the vault, and the window says so first. In a terminal:
`ame vault cert export NAME`. Signing inside the vault is for PDFs and SSH only.

### Mail, your own services and websites

- **Mail.** The address is typed, and nothing checks it. It is exported as a PFX for your mail program, which keeps its own copy;
  no mail program asks the Vault.
- **Logging in to a service** and **This computer's identity** are certificates for a VPN, a Wi-Fi network that wants one, or a server
  of your own, issued by the computer's *Device Identity CA*. The vault exports them as a PFX or as PEM with a locked key, asking
  every time, since NetworkManager and the like read files and cannot ask the Vault. The service has to be told to trust that CA
  (`ame vault cert ca device`). They prove which computer, not who you are.
- **Local TLS** is for a website or device on your own network, by local names (`.local`, `.home.arpa`, `.internal`, `localhost`)
  and private addresses. The root, and the *Local Services CA* under it, can speak for nothing else: a public website's name is
  refused, and a stolen key still could not stand in for one. `ame vault cert export NAME pem` gives a server its key and chain,
  asking every time, since a server cannot ask you. A browser trusts the *Local Services CA* by your choice, never the root:
  `ame vault tls trust browsers on` puts it in the browsers that read your own certificate folder, and the window explains
  how for Firefox and for Flatpak browsers, which keep their own. `curl` and the like take `--cacert`.

### Trusting a certificate

Whoever should accept your signatures has to trust this computer's root, or your CA. **Trusting the root means trusting
everything this computer ever issues, to any account on it, for every purpose it will ever have**, and a stolen root key
could sign in any name an administrator of this computer could. So give people your CA where their program lets them trust
just that, and the root only where it does not.

- `ame vault root > root.pem` writes the root, with its number and fingerprint shown on the terminal for you to compare.
  `ame vault cert ca > ca.pem` writes your CA. Compare the fingerprint with the person you give it to, not by the same route.
- **On Linux**, for a signed PDF: `certutil -d sql:$HOME/.pki/nssdb -A -n "Amethystora" -t ",C," -i ca.pem`, then check with
  `pdfsig -nssdir sql:$HOME/.pki/nssdb signed.pdf`. (Make the folder's database first with `certutil -d sql:$HOME/.pki/nssdb -N --empty-password` if it is not there.)
  Always give `pdfsig` that folder: after Firefox first starts it looks in its own profile instead. Papers, which is a Flatpak with its own, older NSS, may still say "untrusted issuer" for a
  certificate that `pdfsig` accepts, and `pdfsig` is the check to believe.
- **On Windows**, import the file into the *Trusted Root Certification Authorities* store of the account (`certutil -user
  -addstore Root root.pem` in a terminal), which Windows's own programs and `signtool` then use. Adobe's readers keep a list of
  their own, or can be told to use Windows's: this has not been tried here.
- **For Java**, `keytool -importcert -alias amethystora -file ca.pem -keystore truststore.jks`, and check with `jarsigner -verify
  -certs -keystore truststore.jks app.jar`. A Windows program checked with `osslsigncode` needs the root, not the CA.
- **Never put the root in the system's trust store.** It would make it a trust anchor for every program that checks websites.
  The Vault never does, and leaves it to you.

**If the root's key is stolen,** `ame vault root rotate` (asking an administrator) makes a new root, and every account gets a new
CA at its next certificate. Nothing revokes the old one, because nobody runs a server for it: **everyone who trusted the old root
has to stop by hand.** An administrator of this computer can, by becoming the service's user, issue in any name; the root vouches
for this computer and no more.

## Using a secret without handing it over

Most tools need a secret only for a moment. In order of how little leaves the vault:

### The SSH agent

`ame vault ssh on` points your programs at an agent the Vault runs, from your next login (`ame vault ssh status` says how it is).
`ssh`, `git` and `ssh-keygen -Y` then use your SSH keys **without them ever leaving the vault**: each signature asks as the key's
setting says, and the prompt names the program. A key set to **While open** asks again after 15 minutes. A closed vault gives a
program an empty list of keys, so that it moves on to the next way.

- A message that is not a login (`git` signing a commit, `ssh-keygen -Y`) always asks, and says what it is signing for.
- **A forwarded agent** (`ssh -A`) always asks, names the other computer's host key, and shows no key names there.
- `ssh-add -L` lists public keys only. `ssh-add` cannot add or delete anything.
- A Flatpak given `--socket=ssh-auth` reaches it like any program, and the prompt names the app.

### Tools' own helpers

`ame vault helpers` says which tools ask the vault for their logins, and `ame vault helpers git on` (also `docker`, `aws`, `kube`
and `claude`) writes that tool's own settings, only when you ask and never over a setting of someone else's. Git and Docker ask
through their credential helpers; `aws`, `kubectl` and Claude Code ask `ame vault get`. Nothing is written to disk. Git is not
given the vault while another credential helper would keep what the vault answers.

### One command with a secret

`ame vault run NAME -- COMMAND` runs one command with a password as a variable, after you approve, or with a protected file
at its path. **Every program the command starts inherits the variable, and other programs of your account can read it while it runs.**
It is the last resort for a tool that has no helper. A variable that changes how programs start (`LD_PRELOAD`, `PATH` and the
like) is never given a secret.

### Protected files

`ame vault protect PATH` takes a file or folder out of your home and seals it in the vault. The usual places are known
(`~/.ssh/id_*`, `~/.aws/credentials`, an AI agent's login and the like), and `ame vault protected usual` lists what is there and
not yet protected. It is never mounted into your account, since a mount is open to every program of yours, which is what this
exists to prevent.

A tool reaches a protected file through `ame vault run NAME -- COMMAND`: the file appears at its path for that command alone, after
you approve, and is sealed again when the command ends, since tools rewrite their logins (an AI agent when it refreshes its
token). The window and the AI agent launcher do this for you.

**What this does not do, plainly:**

- **While the command runs, other programs of your account can reach the file.** They can read it through `/proc`, enter the
  command's namespace, read its memory and attach to it. A protected file is safe at rest and while nothing uses it, and open to your
  own programs while its tool runs.
- **Old copies survive.** Your hourly snapshots keep the plain file for up to 14 days, and your backups for as long as they last, and
  nothing here deletes from them. The original is unlinked, not wiped, since the disk copies on write. **Replace the key or the
  token after you protect it**: whatever could read the old file may have kept it. The window says so before it does anything.
- **There is no `sudo` or `pkexec` inside a run**: a program's namespace has no setuid programs. An AI agent started this way
  therefore cannot use the agent's root mode. The launcher then starts it without its protected login and says so, rather than
  breaking.
- **Files next to the protected one** can be read, and new ones made, but not deleted, renamed or rewritten through a temporary file
  (`sed -i`, `git config`, `ssh-keygen -R`): do those outside the command. A background program the command started ends with it.
- **A command that does not end well** never replaces the good copy: what it left is sealed beside it under its own name
  (`ame vault protected`), and an empty file never replaces anything.
- Where the system gives a command no namespace of its own, the file stays sealed and the command says so; nothing is lost.

## Sync

`ame vault sync connect ADDRESS` (or **Sync** in the window) connects one Bitwarden or Vaultwarden account, and from then on what the vault
holds that syncs becomes items in that account, and the account's changes come back. It is off until you connect, and the address
is on *What this machine sends* from then on (`ame security sends`).

- **What goes, and how.** Each kind of item is one switch, **Off**, **Plain** or **Sealed**, one click each on the **Sync** page or `ame vault sync kind KIND off|on|sealed`
  (`KIND` is `passwords`, `codes`, `backup-codes`, `keys`, `certificates` or `files`; `on` is plain). A kind's choice covers every item of it, those set to
  **Ask every time** and those added later included, and you are asked once for it. An item's own choice, on its page or with `ame vault sync choose`, wins over its kind's.
  Choosing on an item's page what its kind does takes the item's own away.
  Switching a kind (or an item) from plain to sealed, or off, takes what the vault wrote out of the copy on the server: a note the vault made goes to the server's trash, and a Login another app made is kept with its own folder and fields but without the password, name and codes the vault wrote. The identity certificate of this computer is never covered by the Certificates switch; only its own choice sends it. A file the server lost is sent again by `ame vault sync now accept`.

  | In the vault | Plain: what Bitwarden's apps open | Sealed | Until you choose |
  |---|---|---|---|
  | Password | A Login | A note only this vault opens | Plain |
  | Authenticator code | The `totp` of the matching Login, or a Login of its own | The same note | Plain |
  | Backup codes | A secure note | The same note | Plain |
  | Key | An SSH key item where the server keeps one; any other key a secure note with its private part in a hidden field | The same note | Off |
  | Certificate | A secure note: the chain in its text, the private key in a hidden field, its kind and expiry as plain fields | The same note | Off |
  | Protected file | A secure note named after it, the file as its attachment (a folder as a `.tar` of what it holds) | The same note, with a file | Off |

  Non-exportable keys, the root, your CA's key, the recovery key and a hidden vault never go, whatever is chosen. Turning a kind off, or between
  plain and sealed, moves its copies on the server to the server's trash; nothing is destroyed there.
- **Plain** is Bitwarden's own encryption alone: Bitwarden's apps, its browser extension and every other client of the account open it, which is the point,
  and the server's encryption is all that protects it. A plain item changed in another app comes back as a password does, and when both sides changed
  both are kept. A note of ours that no longer reads as a key or certificate, or a key of another kind than it was, is left as it is here and said, never deleted.
  An SSH key made in another app comes in as a key only while **Keys** is on; a certificate or a file made elsewhere does not, since nothing says it is one.
- **Sealed** is a note holding the item under this vault's own key, so even a broken server learns nothing and Bitwarden's apps cannot read it. Another computer
  needs this vault restored with its recovery key. What does not fit one note goes as the note's file, under the same seal.
- **Files** (what is larger than a note or a field, and every protected file) are attachments of the item's own note, made once and never changed: a changed file
  is a new item on the server and the old one goes to the server's trash. They go only to the server you connected, never to a storage address a server names.
  Vaultwarden keeps them, with no limit but its own. Bitwarden's own servers keep files at a storage host this vault never contacts, so **there only what fits one
  note or field syncs** and the rest stays on this computer, with the reason on the item's page; a Bitwarden server of your own needs premium. If a server
  does not take files, or keeps them elsewhere, the vault says so, asks once and stays quiet until you run `ame vault sync now`.
- **When.** Only while the vault is open: on opening, a little after a change, and every 15 minutes. It waits on a metered connection.
  The server is a copy and the vault is the source: a server that is down stops nothing.
- **Whoever opens the vault can reach that whole account**, because its key is kept in the vault. Use an account of its own for the
  vault. The personal API key is the better login: no master-password hash is sent and no second step is asked; the master password is
  used once and not kept.
- **What the server learns.** How many items there are, of which kind, how long each is and when it changed. It does not see what is in
  them. Plain items are encrypted with the account's key like every item in it, so what a server holds of one is as unreadable to it as a password is, but every app that has the account opens them, and a server that tampers can rearrange them. A server can still try to trick an app, so keep your most sensitive items **sealed**, and use a server you trust.
- **A server you run yourself with its own certificate authority** is trusted for this connection alone, by a fingerprint that you
  compare first.
- **It stops and asks** when a great many items go or come at once (five in one run, or a few a day for a week), or when an account
  asks for more than 256 MiB of memory to make its key. Nothing is written until you say so with `ame vault sync now accept`.
- **Conflicts never lose anything.** An item changed on both sides keeps both, the server's marked as such. A deletion goes to each
  side's trash and is kept for 30 days. If many items vanish from the server at once, the sync stops and asks you.
- **A service in between.** The service that holds your keys has no network. A second one, `amethystora-vault-net`, has the network and
  no key, and sees only what the server sees. It reaches no host but yours.
- **Not supported:** organizations (their items are left alone), Bitwarden's newer encryption (the sync stops and writes nothing),
  folders and Send. **Last seen working:** Vaultwarden 1.37.5. Bitwarden's own server has never been tried here, and
  its protocol is not promised to a program like this one.

`ame vault sync status`, `now`, `off`, `kind KIND off|on|sealed` and `choose KIND ID on|off|sealed` do the rest.

## In your browser

The Vault has no browser extension of its own. Bitwarden's works with the same account that [Sync](#sync) uses, so it fills your
logins and sign-in codes in Firefox and the Chromium family, and the Vault gives the steps. Nothing is installed for you, no file is
written into your browser's folders, and **the image's Firefox stays exactly as it was built.** **In your browser**, in the window,
shows the same steps, with your server's address ready to copy once Sync is connected.

1. **Connect Sync** to your Vaultwarden or Bitwarden server (see [Sync](#sync)).
2. **Install Bitwarden's extension** from your browser's own store: the window has a button for Firefox, for Chrome, Chromium and
   Brave, and for Edge.
3. **Point it at your server.** On the extension's sign-in screen, open the list beside *Logging in on*, choose the self-hosted
   server, paste your server's address as the Server URL and save.
4. **Sign in with the same account** that Sync connected to.

- **What fills there is what syncs as Plain.** Items that sync **Sealed**, and items that do not sync at all, stay on this computer
  and are not in the extension. The Sync page says which kinds are which.
- **A server with a certificate authority of your own** has to be trusted by the browser as well, or the extension cannot reach it.
- **A browser installed as a Flatpak works too**, since it talks to your server and not to this computer.
- **The extension is Bitwarden's**, a trademark of its owner. The Vault works with it, and is not made by or connected with it. What
  it asks for and how it asks is Bitwarden's, and its help pages describe it.
- With **browser protection** on (see [Browser protection](security.md#browser-protection)), Bitwarden's extension is among the ones
  allowed.
- An earlier image let you turn on a small file in your browser's folders for an extension of the Vault's own. The next login takes that file away.

## Backing up and moving

The vault's files are in `/var/lib/amethystora/vault`, not in your home, so neither the hourly snapshots nor the Backups app hold
them, and nothing of your account can delete or swap them.

- **A sealed backup.** `ame vault backup` (or **Back up now** in the window) writes `~/.local/share/amethystora/vault-backup/vault-DATE.avb`:
  everything the vault holds, sealed so that **only the recovery key opens it**. A stolen backup has no password to guess, and writing
  it needs no open vault. The daily backup runs it before each run, and keeps seven. It holds every vault, hidden ones too.
- **Restoring.** `ame vault restore FILE` asks for the recovery key, then for a new way to open the vault.
- **This computer's root and its CAs are in no backup.** A reinstalled computer has a new root, and whoever trusted the old one
  has to be given the new one. A restored vault keeps its old certificates and their chain, which still check out for whoever trusts
  the old root.
- `ame vault forget` removes every vault of your account, and asks you to type `FORGET`.
- Your setup file and its backup never name the Vault, and an image's policy has nothing about it.

## What it does not do

- **While a vault is open with *While open*, a program of your account can ask, without a prompt.** Short times and *Ask every time* are
  the answer.
- **A secret you hand to a program is that program's.** The Vault cannot take it back.
- **A program can draw a window that looks like the vault's prompt** and ask for the password. A security key or a fingerprint resists that;
  a typed password does not.
- **Root can reach everything** that is not sealed under a password, a security key or the recovery key, and even those while the
  vault is open.
- **An old copy of the whole vault put back is not noticed.** An item put back from before is noticed, but a whole older folder
  of the vault, restored from a backup of the disk, opens as it was: nothing outside the disk could tell it from the real one.
- **A program that keeps asking keeps the vault open**, since using it counts as use, and it can make prompts appear. Saying no three
  times a minute to one program quiets it for a minute, and a program can change the name it goes by.
- **It cannot make a certificate the world trusts**, and it does not revoke one. See [Certificates and signing](#certificates-and-signing).
- **It does not keep a timestamp** of what you signed, and a signature may be called expired after its certificate.
- **No sharing** with other people, no organizations' collections, no passkeys for websites, no import from other password managers beyond
  what sync brings in and an existing SSH key.
- **No backup of the machine's root**, and no hosted authority.

## Commands

Every command takes its words as arguments and a secret never as one. `ame vault` lists them; the ones that ask something open a
terminal from the window.

| Command | Does |
| --- | --- |
| `ame vault setup`, `open`, `lock`, `status` | Make, open, close the vault, and say how it is |
| `ame vault methods` | The ways it opens: `add`, `remove`, `idle MINUTES`, `background on|off` |
| `ame vault change-password`, `new`, `rename`, `remove-this`, `forget` | Passwords and vaults |
| `ame vault password`, `codes`, `auth`, `key`, `cert` | Passwords, backup codes, authenticator codes, keys and certificates |
| `ame vault pdf sign FILE`, `root`, `root rotate` | Signing and the machine's root |
| `ame vault ssh on|off|status`, `helpers`, `get`, `run`, `protect`, `protected`, `unprotect` | Using a secret without handing it over |
| `ame vault sync`, `logos`, `tls` | Sync, service logos, trusting the local CA |
| `ame vault backup`, `restore FILE` | The sealed backup |
