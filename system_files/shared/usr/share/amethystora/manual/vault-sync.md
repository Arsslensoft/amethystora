# Sync and backups

Sync keeps a copy of the vault in a Bitwarden or Vaultwarden account of yours; the sealed backup keeps one that only the
recovery key opens.

## Sync

`ame vault sync connect ADDRESS` (or **Sync** in the window) connects one Bitwarden or Vaultwarden account. From then on what the
vault holds that syncs becomes items in that account, and the account's changes come back. It is **off** until you connect, and
from then on the address is on *What this machine sends* (`ame security sends`).

![The vault and your server](media/vault-sync-server.webm)

`ame vault sync status`, `now`, `off`, `kind KIND off|on|sealed` and `choose KIND ID on|off|sealed` do the rest.

### What goes, and how

Each kind is one switch, **Off**, **Plain** or **Sealed**, one click each on the **Sync** page or
`ame vault sync kind KIND off|on|sealed` (`KIND` is `passwords`, `codes`, `backup-codes`, `keys`, `certificates` or `files`; `on`
is plain). You are asked once for a kind, and its choice covers every item of it, those set to **Ask every time** and those added
later included. An item's own choice (its page, or `ame vault sync choose`) wins; choosing on an item's page what its kind does
takes the item's own away.

| In the vault | Plain: what Bitwarden's apps open | Sealed | Until you choose |
|---|---|---|---|
| Password | A Login | A note only this vault opens | Plain |
| Authenticator code | The `totp` of the matching Login, or a Login of its own | The same note | Plain |
| Backup codes | A secure note | The same note | Plain |
| Key | An SSH key item where the server keeps one; any other key a secure note with its private part in a hidden field | The same note | Off |
| Certificate | A secure note: the chain in its text, the private key in a hidden field, its kind and expiry as plain fields | The same note | Off |
| Protected file | A secure note named after it, the file as its attachment (a folder as a `.tar` of what it holds) | The same note, with a file | Off |

- **Never sent**, whatever is chosen: non-exportable keys, the root, your CA's key, the recovery key and a hidden vault. This
  computer's identity certificate is not covered by the Certificates switch; only its own choice sends it.
- **Switching a kind or item** to off, or between plain and sealed, takes what the vault wrote out of the server's copy: a note the
  vault made goes to the server's trash, and a Login another app made keeps its own folder and fields but loses the password,
  name and codes the vault wrote. Nothing is destroyed there.
- A file the server lost is sent again by `ame vault sync now accept`.

### Plain and Sealed

- **Plain** is Bitwarden's own encryption alone: every client of the account opens it (its apps, its browser extension), which is
  the point, and the server's encryption is all that protects it. A plain item changed in another app comes back; changed on both
  sides, both are kept. A note of ours that no longer reads as a key or certificate, or a key of another kind, is left as it is here
  and said, never deleted. An SSH key made in another app comes in as a key only while **Keys** is on; a certificate or file made
  elsewhere does not, since nothing says it is one.
- **Sealed** is a note holding the item under this vault's own key: even a broken server learns nothing, and Bitwarden's apps
  cannot read it. Another computer needs this vault restored with its recovery key. What does not fit one note goes as the note's
  file, under the same seal.

### Files

Files (anything larger than a note or field, and every protected file) are attachments of the item's own note, made once and
never changed: a changed file is a new item, and the old one goes to the server's trash. They go only to the server you connected,
never to a storage address a server names.

- Vaultwarden keeps them, with no limit but its own.
- Bitwarden's own servers keep files at a storage host this vault never contacts, so **there only what fits one note or field
  syncs**; the rest stays here, with the reason on the item's page. A Bitwarden server of your own needs premium.
- If a server does not take files, or keeps them elsewhere, the vault says so, asks once and stays quiet until `ame vault sync now`.

### When it syncs, and what the server learns

- **Only while the vault is open**: on opening, a little after a change, and every 15 minutes. It waits on a metered connection.
  The vault is the source and the server a copy: a server that is down stops nothing.
- **Whoever opens the vault can reach that whole account**, because its key is kept in the vault. Use an account of its own for the
  vault. The personal API key is the better login: no master-password hash is sent and no second step is asked; a master password
  is used once and not kept.
- **The server learns** how many items there are, of which kind, how long each is and when it changed, not what is in them. Plain
  items are as unreadable to it as a password, but every app of the account opens them, and a tampering server can rearrange them
  or try to trick an app. Keep your most sensitive items **sealed**, and use a server you trust.
- **A server with its own certificate authority** is trusted for this connection alone, by a fingerprint you compare first.

### When it stops and asks

- When many items go or come at once (five in one run, or a few a day for a week), or an account asks for more than 256 MiB of
  memory to make its key. Nothing is written until `ame vault sync now accept`.
- **Conflicts never lose anything.** Changed on both sides, both are kept, the server's marked. A deletion goes to each side's
  trash for 30 days. If many items vanish from the server at once, it stops and asks.

### Behind it, and what is not supported

- **A service in between.** The service holding your keys has no network. A second one, `amethystora-vault-net`, has the network and
  no key, sees only what the server sees, and reaches no host but yours.
- **Not supported:** organizations (their items are left alone), Bitwarden's newer encryption (the sync stops and writes nothing),
  folders and Send. **Last seen working:** Vaultwarden 1.37.5. Bitwarden's own server has never been tried here, and its protocol
  is not promised to a program like this one.

## Backing up and moving

The vault's files are in `/var/lib/amethystora/vault`, not your home, so neither the hourly snapshots nor the Backups app hold them,
and nothing of your account can delete or swap them.

- **A sealed backup.** `ame vault backup` (or **Back up now**) writes `~/.local/share/amethystora/vault-backup/vault-DATE.avb`:
  everything the vault holds, sealed so that **only the recovery key opens it**. A stolen backup has no password to guess, and
  writing one needs no open vault. The daily backup runs it before each run and keeps seven. It holds every vault, hidden ones too.
- **Restoring.** `ame vault restore FILE` asks for the recovery key, then for a new way to open the vault.
- **This computer's root and its CAs are in no backup.** A reinstalled computer has a new root, which whoever trusted the old one
  must be given. A restored vault keeps its old certificates and chain, still valid for whoever trusts the old root.
- `ame vault forget` removes every vault of your account, after you type `FORGET`.
- Your setup file and its backup never name the Vault, and an image's policy has nothing about it.
