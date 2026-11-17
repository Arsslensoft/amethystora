# An agent held to its project

A sandbox keeps an [agent](ai-agent.md) inside its project and away from your SSH keys and logins. Started in a folder with a rule, the agent and
everything it runs can write in the project and nothing else, never see your SSH keys, tokens and logins, and cannot become root. It is off until you
turn it on for a folder, and it costs nothing while off.

## Turning it on

```bash
cd ~/code/scraper
ame agent sandbox use              # this folder, an open sandbox
ame agent sandbox use ~/code/api closed
ame agent sandbox off ~/code/api   # take the rule away
ame agent sandbox list             # every folder with a rule
ame agent sandbox show             # what an agent started here would be given
```

From then on an agent started in that folder, or below it, runs inside. It starts with three lines: the project, what is
hidden from it, and that the network is open. **A project never picks its own sandbox:** the rule is yours alone, and nothing is read from the repository. For one launch alone, `amethystora-agent --sandbox` turns it on and
`--no-sandbox` turns it off.

## What it can write

- The project: the top of the git repository it was started in, else the folder. A worktree's repository folder too.
- The agent's own folder (the [profile](agent-profiles.md)'s).
- The build caches that exist: Cargo's, npm's, Go's, pip's, uv's, Maven's and Gradle's.
- A `/tmp` and `/var/tmp` of its own, removed when it ends.

Everything else of the machine and your home is there to read and cannot be written.

## What it cannot see

An empty folder or file stands in its place for your logins and keys (`~/.ssh`, `~/.gnupg`, the cloud tools' and `gh`'s
settings, the container tools' logins, keyrings, `.npmrc`, `.netrc`), browsers' profiles, other apps' data, the Notes data,
`~/Private`, the shells' histories, every other agent's folder and every other profile, and the hourly snapshots of your
home, which hold copies of `~/.ssh`. There is no session, no keyring, no ssh agent, no [Vault](vault.md), no display and
no container socket. It sees only its own processes, no graphics card, USB or camera, and `sudo`, `pkexec` and rootless
containers do not work.

Inside the project, the repository's hooks and settings (`.git/hooks`, `.git/config`) and every agent's settings
(`.claude`, `.mcp.json`, `.codex`, `.cursor`, `.gemini`, `.opencode`, `opencode.json`, `.copilot`) are read-only, and so are
the files in the agent's own folder that start commands or loosen its checks. Its environment is yours, less the session's
sockets and every variable whose name says secret (`TOKEN`, `PASSWORD`, `*_KEY`...), except the agent's own key.

If a sandbox cannot be made, nothing starts, and the sentence says why.

## What it is not

- It is not a virtual machine: the kernel is shared.
- **It does not make what the agent wrote safe.** A build script or a test it changed runs as you when you run it: read
  the diff first. When the agent ends, you are told if it made a settings file such as `.mcp.json`, which an agent reads
  when it next starts outside a sandbox.
- It does not hide the project's own secrets, such as a `.env` in it.
- It does not hold an agent that an editor's extension starts.
- The network is the machine's unless you [hold it to a list](#the-network): it can still send what it can read.
- **It can still talk to programs of yours that listen.** A program that listens on a socket in your home, such as an editor's
  server, can be reached there, since connecting needs no writing. While the network is the machine's it can also reach
  what listens on this computer (`localhost`) and the display server of older apps (X11), which can watch and type into
  those apps. Holding the network to a list closes the last two.

## Allowing more

A folder may be given more, one permission at a time. Each is asked for in a terminal, after it says what it gives away.

```bash
ame agent sandbox allow ~/code/api write=~/data/exports   # a second place it may write
ame agent sandbox allow ~/code/api show=~/.config/tool    # a hidden place, shown read-only
ame agent sandbox allow ~/code/api ssh-agent              # log in with the ssh agent's keys, for git push
ame agent sandbox allow ~/code/api gpu
ame agent sandbox allow ~/code/api network=api.example.org   # one more host, while the network is held to a list
ame agent sandbox disallow ~/code/api gpu
```

`ssh-agent` lets the agent use every key the ssh agent holds, without reading one.

## The network

As it starts, a sandbox has your computer's network. A folder can hold it to a list instead:

```bash
ame agent sandbox network ~/code/api list   # only the hosts on the list
ame agent sandbox network ~/code/api open   # the computer's network again
ame agent sandbox show ~/code/api           # says which hosts
```

The list is the agent's own maker, the package registries (npm, PyPI, crates.io, Go, Maven) and the code hosts (GitHub,
GitLab, Bitbucket, Codeberg), plus what you allow with `network=HOST`. Only HTTPS, and only by name. Everything else is
refused: another site, an address, plain HTTP, and anything on this computer or your home network.

It works through a proxy, which most tools use by themselves (the agents, `git` over HTTPS, `npm`, `pip`, `cargo`,
`curl`). **A program that ignores the proxy reaches nothing**, and `git` over SSH is one: use an HTTPS address for the
remote. A development server or database on `localhost` outside the sandbox cannot be reached; one started inside can.
**A list narrows where things can go; it does not stop them.** The proxy sees a host's name and nothing of what is sent, so
an agent that was talked into it can still send what it reads to its maker, or to somebody's account on a code host or a
package registry on the list.

## A closed sandbox

`ame agent sandbox use FOLDER closed` shows it none of your home but the project, its own folder, the caches and the
toolchains (`~/.cargo/bin`, `~/.rustup`, `~/.local/bin`, `~/.gitconfig`), read-only.

## Codex and Copilot CLI

**Codex** installs a helper program into `~/.codex/packages` the first time it runs, which a sandbox cannot do: started
first in a sandbox it says "Read-only file system". Start Codex once outside a sandbox, then it works inside.

**Copilot CLI**'s login is in the keyring, which a sandbox hides. Inside one, it needs its token in `COPILOT_GITHUB_TOKEN`
or `GH_TOKEN`.

## A program nobody has read

```bash
ame agent sandbox run -- ./install.sh
```

runs any command held as an agent would be, with no agent's folder: a good way to try an install script. It
has the same limits as above.

## On every machine

`ame agent sandbox always on` is an administrator's setting (it asks for a password): every agent started in a project is
sandboxed and `--no-sandbox` is refused. It binds the launcher and the shell's hook only, since an agent is a file in your
home that anyone can run. `ame agent sandbox hook on` makes your shell send `claude`, `codex` and the rest through the
folder's profile and sandbox (see [Several accounts](agent-profiles.md#in-your-shell)).

## When the agent says "Read-only file system"

Outside its project it is the sandbox, not a fault. The agent will say so. A missing login, key or session bus is the
same. Run `ame agent sandbox allow` yourself if you want it to reach that place, or leave it. Nothing of the sandbox is in
the [setup file](setup.md).
