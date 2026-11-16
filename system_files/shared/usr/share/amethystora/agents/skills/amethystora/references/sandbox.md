# Running in a sandbox, and agent profiles

## Are you in a sandbox

If the variable `AMETHYSTORA_AGENT_SANDBOX` is set (`open` or `closed`), you and everything you start run inside one, held to your
project, which `AMETHYSTORA_AGENT_SANDBOX_PROJECT` names. You can write in the project, your own folder, the build caches and a private `/tmp`.
The person's keys and logins are hidden, every other place is read-only, and there is no sudo, no session bus, no keyring, no ssh agent and
no display. `ame agent sandbox show` says what you are given.

## When something is refused

"Read-only file system" outside the project, a missing key or login (`~/.ssh`, `gh`, a cloud tool), no session bus, no sudo: these are the
sandbox working, not faults on the machine. Do this:

- Say it to the person in one sentence: what you could not reach and that the sandbox holds it back.
- If they want you to reach it, name the command for them to run, never to run yourself: `ame agent sandbox allow FOLDER PERMISSION`, where
  PERMISSION is `write=PATH` (a second place to write), `show=PATH` (a hidden place, read-only), `ssh-agent` (`git push` with the ssh agent's keys)
  or `gpu`. It asks them in a terminal and says what it gives away. Plain `git push` and a login you need are theirs to arrange.
- **Never look for a way round.** No other path to the same file, no copying it somewhere you can write, no second agent, no other program
  or container to do it, no editing the launcher or a setting. Whatever it was held back from, the person decides.

If `HTTPS_PROXY` points at `127.0.0.1:3128`, the sandbox's network is held to a list of hosts: your maker, the package registries and the
code hosts, over HTTPS. A `403` from the proxy, a name that does not resolve, `git` over SSH that hangs or a `localhost` service that is
not there are the list, not a fault. Say which host you needed; the person may run `ame agent sandbox allow FOLDER network=HOST`, or use an
HTTPS address for a git remote. Never try another route out.

A file that stays read-only inside the project (`.git/hooks`, `.git/config`, `.claude`, `.mcp.json`, `.codex`, `opencode.json`...) is held on
purpose: tell the person what you would have changed and let them make it. A script you changed runs as them when they run it: say which, so
they read the diff.

Codex's first start needs its program in `~/.codex/packages`: if it says "Read-only file system" there, the person starts Codex once outside a
sandbox. Copilot CLI inside one needs its token in `COPILOT_GITHUB_TOKEN` or `GH_TOKEN`.

You may run `ame agent sandbox show [FOLDER]` and `ame agent sandbox list`. Turning a sandbox on or off, allowing, disallowing and
`always` are the person's (and the last an administrator's).

## Profiles

A profile is another login of the same agent. You never make, pick or change one: never set `CLAUDE_CONFIG_DIR`, `CODEX_HOME` or
`AMETHYSTORA_AGENT_PROFILE_PIN`, in front of a command or anywhere else, and never start yourself under another account. The tools refuse it, and
a folder or repository cannot choose one either. To tell the person which account a folder uses, run `ame agent profile which [FOLDER]`;
`ame agent profile list` and `ame agent profile running` list them and the sessions. Making one (`add`, `login`), using one for a folder
(`use`) and removing one are the person's. A profile does not share one subscription between people.
