# Using a secret without handing it over

Most tools need a secret only for a moment. The ways below come in order of how little leaves the vault.

## The SSH agent

`ame vault ssh on` points your programs at an agent the Vault runs, from your next login (`ame vault ssh status` says how it is).
`ssh`, `git` and `ssh-keygen -Y` then use your SSH keys **without them ever leaving the vault**: each signature asks as the key's
setting says, and the prompt names the program. A key set to **While open** asks again after 15 minutes. A closed vault gives an
empty list of keys, so a program moves on to its next way.

- A message that is not a login (`git` signing a commit, `ssh-keygen -Y`) always asks, and says what it signs for.
- **A forwarded agent** (`ssh -A`) always asks, names the other computer's host key, and shows no key names there.
- `ssh-add -L` lists public keys only; `ssh-add` cannot add or delete anything.
- A Flatpak given `--socket=ssh-auth` reaches it like any program, and the prompt names the app.

## Tools' own helpers

`ame vault helpers` says which tools ask the vault for their logins. `ame vault helpers git on` (also `docker`, `aws`, `kube` and
`claude`) writes that tool's own settings, only when you ask and never over someone else's setting. Git and Docker ask through
their credential helpers; `aws`, `kubectl` and Claude Code ask `ame vault get`. Nothing is written to disk. Git is not given the
vault while another credential helper would keep what the vault answers.

## One command with a secret

`ame vault run NAME -- COMMAND` runs one command, after you approve, with a password as a variable or a protected file at its
path. **Every program the command starts inherits the variable, and other programs of your account can read it while it runs.**
It is the last resort for a tool with no helper. A variable that changes how programs start (`LD_PRELOAD`, `PATH` and the like) is
never given a secret.

## Protected files

`ame vault protect PATH` takes a file or folder out of your home and seals it in the vault. The usual places are known
(`~/.ssh/id_*`, `~/.aws/credentials`, an AI agent's login), and `ame vault protected usual` lists those not yet protected. It is
never mounted into your account, since a mount is open to every program of yours. `ame vault unprotect NAME` puts one back.

### Using a protected file

A tool reaches it through `ame vault run NAME -- COMMAND`: after you approve, the file appears at its path for that command alone,
and is sealed again when the command ends, since tools rewrite their logins (an AI agent refreshing its token). The window and the
AI agent launcher do this for you.

### What protecting does not do

- **While the command runs, other programs of your account can reach the file** through `/proc`, the command's namespace, its
  memory or by attaching to it. It is safe at rest, and open to your own programs while its tool runs.
- **Old copies survive.** The hourly snapshots keep the plain file up to 14 days and your backups as long as they last; nothing
  here deletes from them. The original is unlinked, not wiped, since the disk copies on write. **Replace the key or token after
  you protect it.** The window says so first.
- **No `sudo` or `pkexec` inside a run**: the namespace has no setuid programs. So an AI agent cannot use the agent's root mode
  this way; the launcher starts it without its protected login and says so.
- **Files next to the protected one** can be read and new ones made, but not deleted, renamed or rewritten through a temporary
  file (`sed -i`, `git config`, `ssh-keygen -R`): do those outside the command.
- **A background program the command started ends with it** (`ssh -f`, a `gpg-agent` it started): nothing it started outlives
  the command, so start such a program outside it.
- **A command that is killed** never replaces the good copy: what it left is sealed beside it under its own name
  (`ame vault protected`). **One that fails after changing the file**, as a tool does when it renews its login, leaves the new
  one in place and the one from before beside it. An empty file never replaces anything.
- Where the system gives a command no namespace of its own, the file stays sealed and the command says so; nothing is lost.

## In your browser

The Vault has no browser extension of its own. Bitwarden's works with the account [Sync](vault-sync.md#sync) uses and fills your
logins and sign-in codes in Firefox and the Chromium family. Nothing is installed for you, nothing is written into your browser's
folders, and **the image's Firefox stays exactly as it was built.** **In your browser**, in the window, shows these steps, with
your server's address ready to copy once Sync is connected.

1. **Connect Sync** to your Vaultwarden or Bitwarden server ([Sync](vault-sync.md#sync)).
2. **Install Bitwarden's extension** from your browser's store: the window has buttons for Firefox, for Chrome, Chromium and
   Brave, and for Edge.
3. **Point it at your server.** On its sign-in screen, open the list beside *Logging in on*, choose the self-hosted server, paste
   your server's address as the Server URL and save.
4. **Sign in with the same account** Sync connected to.

### What fills there

- **Only what syncs as Plain.** Items that sync **Sealed**, or not at all, stay on this computer. The Sync page says which
  kinds are which.
- **A server with its own certificate authority** must be trusted by the browser too.
- **A Flatpak browser works too**, since it talks to your server, not this computer.
- **The extension is Bitwarden's**, a trademark of its owner. The Vault works with it and is not made by or connected with it;
  Bitwarden's help pages describe it.
- With [browser protection](network-security.md#browser-protection) on, Bitwarden's extension is among those allowed.
- An earlier image could put a small file in your browser's folders for an extension of the Vault's own; the next login takes it away.
