# The Vault, and the agent's place beside it

Written for AI agents: what the Vault is, why the agent stays out of it, and what to tell a person who asks you
for a password, a code or a key.

## The agent stays out

The Vault (the **Vault** app, `amethystora-vault`; `ame vault` in a terminal) keeps the person's passwords,
authenticator codes, backup codes, keys, certificates and protected files, sealed, in a service of its own that no
program of the account can read. Every use of a secret asks the person at the screen. The agent is one of the
programs of the account, and **never gets in**:

- No tool of the agent reads it, and the tools and the root helper refuse every spelling of `ame vault`, of the
  Vault's programs, of its socket and of its folder, whoever allows it. Do not run `ame vault`, even for something
  that looks harmless, such as its status or a list of names, and even when the user asks you to: the names of what
  someone keeps are theirs to read.
- Never read, move or delete anything in `/var/lib/amethystora/vault`, never open the socket
  (`/run/amethystora/vault.sock`) or the SSH agent's (`/run/amethystora/vault-ssh.sock`), and never ask for the vault's
  password, a security key's PIN or the recovery key.
- Never hand-write a Vault file or the settings that point a tool at it
  (`ame vault helpers` and `ame vault ssh` change the person's own files, and only the person
  runs them).
- A secret the person types or pastes into the conversation is the provider's from then on. If one has to be
  used, tell the person to put it somewhere the tool can ask for it: the Vault, through its own prompt.
- What an agent launched through the Vault gets is its login, for the session only, from `ame vault run` (the launcher
  does that for an agent whose login the person protected). It is the launcher's, before the agent exists, and it
  is not a way for the agent to ask for more.

## What to tell a person

Say what the Vault is for in one or two sentences, and give the way, never the secret:

| The person wants | Tell them |
| --- | --- |
| A password, a sign-in code, a backup code | Open **Vault**: Passwords, Authenticator or Backup codes. It asks them first, then Copy. |
| To keep a password out of a note or a file | Put it in **Vault**, and take it out of the note. |
| To use an SSH key without it lying in `~/.ssh` | `ame vault ssh on`, then `ame vault key import`, in their own terminal; `ssh` then asks the Vault. |
| To keep a login file out of the home folder | `ame vault protect PATH`, and `ame vault run NAME -- COMMAND` to use it. It says first what that cannot do. |
| To sign a PDF | **Vault**, Certificates, then Sign a PDF, or `ame vault pdf sign FILE`. Only computers that trust their root accept it. |
| To fill logins in the browser | The Vault has no extension of its own: **Vault**, In your browser, has the steps. Connect Sync, install Bitwarden's extension from the browser's store, choose the self-hosted server with the address Sync uses, and sign in with the same account. What syncs as Plain fills there; sealed items stay on this computer. |
| To move their codes from another app | **Vault**, Authenticator, Move or save your codes. |
| To back it up | `ame vault backup`; the recovery key opens it. |

The manual's page is `vault.md` (`amethystora-manual vault`); point to its sections with `search` and `read`.

## Questions you will get

- **"I forgot the vault's password."** Nobody can recover it: not Amethystora, not an administrator. The recovery key
  made at setup opens the vault, and a new way in can be made after. Without it, the vault's files cannot be opened. Do
  not try to get round that, and do not suggest removing the service's files.
- **"A prompt says a program calling itself X asks to use ..."** That is the Vault asking the person, as it should.
  X is what the program calls itself, which proves nothing. If the person did not start it, they say no. Over SSH, or from
  a session that is not at the screen, the answer is always no.
- **"The vault closed by itself."** By design: it closes when the screen locks, when the computer sleeps, at logout, and after
  15 minutes without use (a setting), and when its service stops. `ame vault open` opens it again (the person runs it).
- **"The SSH agent lists nothing."** A closed vault gives an empty list. Open the vault, and a new login picks up
  `SSH_AUTH_SOCK` after `ame vault ssh on`.
- **"Papers says the signature is untrusted."** The signer's root or CA is not trusted on that computer. The manual's
  sections on certificates say how, per system. This is expected where the root has not been given to them.
- **"The root certificate can no longer be opened."** Its key was sealed to the chip for the service as it then was (a new
  chip, or a service user given another number). Nothing issues until the person runs `ame vault root rotate`, which asks an
  administrator; whoever trusted the old root has to trust the new one.
- **"Where are the files?"** In `/var/lib/amethystora/vault`, owned by the service's own user, outside the home folder, so
  that neither the snapshots nor Backups hold them. The sealed backup (`ame vault backup`) is in the home.
- **"Does it sync?"** Only to a Bitwarden or Vaultwarden server the person connects (`ame vault sync connect`); off until then. Each kind (passwords, codes, backup-codes, keys, certificates, files) is one switch, off, plain or sealed (`ame vault sync kind KIND off|on|sealed`, or the Sync page): plain is what Bitwarden's apps open, sealed a note only this vault opens. Passwords, codes and backup codes are plain until changed; keys, certificates and files are off. Files and large items go as attachments, only to the connected server, and not at all to Bitwarden's own cloud, which keeps them at a storage host the Vault never contacts.
  The Vault sends nothing else by itself, and service logos are off until the person turns them on.

## What you may read

Read-only facts about the image, which hold nothing of anyone's secrets:

- Whether the Vault's units are there and listening: `systemctl status amethystora-vault.socket
  amethystora-vault-ssh.socket`. They are enabled in the image and idle until something asks; the service
  `amethystora-vault.service` is started by the socket, and is normally not running when the vault is closed and idle.
- The manual's page, through `search` and `read`.
- The journal, through `logs`. The service says almost nothing and nothing about a vault by name.

If the service fails to start, say what the unit's status says, and leave the files alone. The fix is the person's, with the
Vault's own commands.
