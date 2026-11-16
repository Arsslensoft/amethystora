# Several accounts of one agent

A profile is one more login of the same [agent](ai-agent.md), with its own history and settings: a work account of
Claude Code beside your own, say. Each folder can use its own profile, and any number of terminals can run
different ones at once. With no profile made, nothing changes.

## Making a profile

A profile has a name (lower-case letters, digits and `-`, at most 32 characters). Its folders are in
`~/.local/share/amethystora/agent-profiles/NAME/`, laid out like your home's. The home's own agent folders are the profile
`default`, which cannot be removed.

```bash
ame agent profile add work          # make the profile
ame agent profile login work claude # sign in to Claude Code on it, in a terminal
ame agent profile list              # the profiles, and what each one holds
```

`login` without an agent names the one you chose. Signing in is the agent's own, as it always was.

## Which profile a folder uses

Tell Amethystora once that a folder uses a profile. Every agent started in it, or in a folder below it, gets that profile.

```bash
ame agent profile use work ~/code/client   # this folder and everything under it
ame agent profile use default ~/code/client # take the folder off again
ame agent profile which ~/code/client       # which profile an agent started there would get
```

The choice is yours alone, kept in `~/.config/amethystora/agent-profiles.json`. **A project never picks its own
profile:** nothing is read from the repository, so a folder you cloned cannot send an agent to another account.

### How one is picked

A launch gets the first that applies: `--profile NAME` (`amethystora-agent --profile work`), then the profile this
terminal was pinned to, then the folder's, then `default`. Of several folders that match, the deepest wins.

## Two at once

A profile belongs to one launch and nothing wider. No file says which profile is "current", so two terminals can run two
accounts at the same moment, and a running agent never changes account.

```bash
ame agent profile terminal work ~/code/client   # a terminal in that folder, on that profile
ame agent profile running                       # the sessions running now, and the profile of each
```

The question you ask in the [pill](agent-desktop.md#asking-in-the-pill) goes to `default`, or to the profile `ame agent profile ask NAME` names, never to the focused window's.

The terminal's title says which one it is, such as **Claude Code · work**. Started from the key or the menu, which have
no folder, the launcher asks which profile when you have more than one.

## What it reaches, agent by agent

| Agent | How a profile reaches it |
| --- | --- |
| Claude Code | `CLAUDE_CONFIG_DIR`, which moves `~/.claude` and `~/.claude.json` |
| Codex | `CODEX_HOME`, which moves `~/.codex` |
| Opencode, Cursor CLI, Antigravity CLI | The profile's folders are put over the home's for that launch alone |
| Copilot CLI | No profiles: its login is in the keyring, one for each computer account |

The last three run in a space of their own where nothing can become root, so the agent's root commands are not
available on a profile for them.

## In your shell

`ame agent profile hook on` makes your shell follow the folder. Each time you change folder, the prompt shows `agent work`,
and `claude`, `codex` and the others you type there go through that folder's profile (and its
[sandbox](agent-sandbox.md)). It works in bash, zsh and fish, and is off until you turn it on.

```bash
agent-profile work   # pin this shell to a profile
agent-profile off    # let the folder decide again
agent-profile        # say which
```

`command claude` goes round the hook: it is a convenience, not a lock.

## What it does not do

- **A profile does not share one subscription between people.** Each is a login of its own, with whatever its maker's
  terms say about that. Use it for your own accounts.
- **An agent never changes account by itself.** It cannot make, pick or switch a profile, and cannot set
  `CLAUDE_CONFIG_DIR` or `CODEX_HOME` for itself: Amethystora's tools refuse it. It may say which profile a folder uses
  (`ame agent profile which`).
- **A login protected in the [Vault](vault.md) is shared by the terminals that use it.** Each terminal asks you, as the
  first did. The login stays open for as long as any of them runs, and is sealed again when the last has ended, which may be after the first terminal has its prompt back. If the
  vault is closed by then, a notice says so and the login waits in memory until you open it.
- **Nothing of it is in the [setup file](setup.md):** a profile holds logins, which a shared file must not.

[Security](security.md) reads every profile's agent settings as it reads the home's, and a finding names the profile in
its path.

## Removing one

```bash
ame agent profile remove work
```

It asks first, and refuses while an agent runs on the profile. The folder goes to the trash, where you can still take it
back. The folders it covered follow `default` again.
