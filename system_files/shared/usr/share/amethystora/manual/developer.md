# Developer mode

`amethystora-dx` is the same desktop with a developer's workshop on top: container engines,
virtual machines, VSCodium, and the profiling tools to find out why something is slow.

The idea behind it: your development environment should not be your operating system. Toolchains,
language runtimes and databases live in containers that are described in the project's own
repository, so the project builds the same on your machine, a colleague's Mac and in CI, and the
system underneath stays clean enough to update without a second thought.

## Turning it on

Switch to the dx image, and restart:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora-dx:stable
```

If you have [layered packages](software.md#layering-the-last-resort), use
`sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora-dx:stable` instead,
which keeps them. Switching back to `amethystora` works the same way. The **Developer mode** switch on
[Control](control.md)'s **System** page does it for you, on the stream and driver you are on, keeping layered packages
(`ame system devmode on` or `off`; without either it switches to the other). It asks first, and on
the way in offers extra monospace fonts from Homebrew.

## Groups

Docker, Incus and the system's virtual machines answer only to `sudo` until your account joins
their groups, and the image joins nobody by itself:

```bash
ame system dx-group
```

It lists each group with what it lets you do, and you pick the ones your account should be in; the
same command takes you out of one. `ame system dx-group docker dialout` puts your account in exactly
those and takes it out of the others, and `ame system dx-group none` out of every one; it still asks
before one of the first three below. **Developer groups** on [Control](control.md)'s **System** page
opens it. Log out and back in afterwards.

| Group | Lets your account |
| --- | --- |
| `docker` | Use Docker without `sudo` |
| `incus-admin` | Use Incus without `sudo` |
| `libvirt` | Run virtual machines on the system connection (`qemu:///system`) |
| `dialout` | Use serial adapters. Arduino and Raspberry Pi Pico boards work without it |
| `wireshark` | Capture network traffic in Wireshark |

The first three are root under another name: Docker can start a container that mounts the whole
system, Incus can make privileged containers, and a system virtual machine can be given any disk. Any
program running as you, a script from the internet included, can then take over the machine without
your password, which `sudo` would otherwise ask for. The security report names the accounts in them.
Podman, which is rootless, and virtual machines on your own connection (`qemu:///session`, the
default for GNOME Boxes) need no group at all.

## What it adds

| Tool | For |
| --- | --- |
| VSCodium | The editor: VS Code built from its open source code, without Microsoft's telemetry or licence. Extensions come from [Open VSX](https://open-vsx.org); Open Remote SSH is installed, as published by its maker. Its colours follow the [theme](themes.md). It updates with the system, never by itself. |
| Docker Engine | With buildx, compose and `docker model` for local AI models. Its socket is on, so it starts when first used. The default for dev containers. |
| Podman | With `podman-compose`, `podman machine` and `podman-tui`. Always there, rootless, and the Podman socket is on. |
| qemu-user-static | Containers and programs built for other processors, such as `podman run --platform linux/arm64` |
| Incus | System containers and virtual machines, managed like cloud instances |
| libvirt and virt-manager | Virtual machines with QEMU and KVM; virt-v2v converts machines from other hypervisors |
| Cockpit's bridge | Manage this machine from Cockpit on another one, over SSH |
| VFIO | Loaded at the start, so that a graphics card or another PCI device can be handed to a virtual machine once kernel arguments bind it |
| Sysprof, perf, bcc, bpftrace, bpftop, bpftool, trace-cmd | Profiling and tracing, from the whole system down to one function |
| gdb, strace, ltrace, Valgrind | Debugging: step through a program, watch the calls it makes to the kernel and to its libraries, find memory errors. Attaching gdb to a program it did not start needs `sudo` ([why](security.md#on-from-the-start)). |
| Wireshark | Capturing and reading network traffic, without `sudo` once you join its [group](#groups) |
| android-tools | `adb` and `fastboot` |
| iotop, tiptop, nicstat, numactl | What the disks, processors and network are doing, and which memory a program runs on |
| wtype, ydotool | Type and click from a script; ydotool needs `ame security virtual-input off` |
| udica | SELinux policies made to measure for a container |
| git-svn, git-subtree, 7zip, genisoimage, virt-viewer | The rest of a workshop: Subversion, subtrees, 7-Zip, ISO images, and the window onto a virtual machine |
| ROCm | GPU compute on AMD graphics |
| flatpak-builder | Building Flatpaks |
| Podman Desktop, Builder, Clapgrep, Embellish, DevToolbox | Apps from Flathub, installed with developer mode: containers in a window, GNOME's IDE, searching text across files, Nerd Fonts, and encoders, formatters and generators. Leaving developer mode takes them away again |

Packages published minutes ago are how most poisoned releases reached people:
`ame security package-cooldown on` has npm, pnpm and uv refuse versions less than 3 days old, and
`strict` waits 7 and runs no install scripts. `ame security vscodium-extensions on` stops VSCodium updating extensions by
itself, and installs only the extensions or publishers listed in `/etc/amethystora/vscodium-extensions`
when you list any ([Stricter settings](security.md#stricter-settings)).

## Dev Containers

A dev container is a `.devcontainer/devcontainer.json` in the project: the image, the tools and the
editor extensions the project needs. Everything the project installs stays in there.

Microsoft's Dev Containers extension only works in Microsoft's own VS Code, so on Amethystora dev
containers run through the `devcontainer` command line, the reference implementation of
[the specification](https://containers.dev), which JetBrains IDEs follow too:

```bash
brew install devcontainer
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

To use Podman instead of Docker, add `--docker-path podman --docker-compose-path podman-compose` to
`devcontainer up`.

If a container cannot read a mounted folder because of SELinux, relabel the folder rather than
turning SELinux off: `restorecon -R -v ~/code/myproject`.

## Other editors

- **JetBrains:** `ame apps jetbrains-toolbox` installs the JetBrains Toolbox into your home, which then
  installs and updates the IDEs. The IDEs' Flatpaks are not recommended.
- **Neovim, Helix and friends:** `brew install neovim`, and `brew install devcontainer` for the dev
  container command line.

## Kubernetes and the cloud

Install the command line tools with Homebrew, so they stay current without touching the image:

```bash
brew install kubectl helm k9s kind
```

`kind` runs a whole Kubernetes cluster in Docker containers, which is the quickest way to try
something against a real cluster. `ame apps cncf`, **Cloud native tools** on [Control](control.md)'s
**Apps** page, lets you pick from the Cloud Native Computing Foundation's projects
([Homebrew](software.md#command-line-tools-homebrew)).

Your SSH keys, Git and Docker logins, AWS and Kubernetes credentials and an AI agent's login can stay in the
[Vault](vault.md#using-a-secret-without-handing-it-over) and be handed to each tool only when it asks, with your say-so.

## An image of your own

For what has to be in the system itself, such as a driver, a system service or a tool every account
on the machine needs, build an image of your own on top of Amethystora rather than
[layering](software.md#layering-the-last-resort) packages. The
[image template](https://github.com/arsslensoft/amethystora-image-template) is a repository to start
from: a `Containerfile` that builds `FROM` Amethystora, a script where your packages and files go, and
a GitHub workflow that builds the image whenever you push and once a day, signs it with a key of your
own and publishes it to your account's registry. Renovate moves it onto each new Amethystora release.
Its build ends with `/usr/libexec/amethystora-image-check`, which fails it if a step of yours undid
something Amethystora relies on: the checking of updates, the `ame` commands, or the absence of
Fedora's logo and release packages.

The same repository takes the apps, look and policy every machine and account of yours starts with,
with no script: a [setup file](setup.md) (`ame setup save setup.json` writes this machine's) becomes the
image's defaults, a `branding/` folder gives it your name, wallpaper, logo, login screen, desktop style,
icons and fonts (About says "*your name*, built on Amethystora"), a `policy/` folder sets how far the AI agent
may go, Security's switches, the stricter hardening, GNOME's settings by default or locked for everyone (the
screen lock, a notice on the login screen, the camera, printing, keyboard layouts and more), when machines
update, and the company's proxy, DNS servers, VPN connections and Flatpak remotes, never with a password or a
key in them (each person gives those as they connect), and a `packages` file adds Fedora's packages the system
itself needs. The [image editor](https://amethystora.org/build) on Amethystora's website writes these
files for you. A **Build ISO** workflow in the repository makes the two ISOs of your image, with
Amethystora's own installer files: the one that starts from a USB stick to try it, and the installer.

Your image is signed with your key, not Amethystora's, so tell this machine to take it only with your
signature before you switch to it. `cosign.pub`, from your repository, is the public half of the key:

```bash
ame system trust-image ghcr.io/<you>/<your-image> cosign.pub
sudo bootc switch --enforce-container-sigpolicy ghcr.io/<you>/<your-image>:stable
```

[Control](control.md)'s **System** page does the first line for you: type the repository (in lower case, with its registry, without a tag; Amethystora's own are refused),
choose the `cosign.pub`, and it opens in a terminal, where it asks for your password. Once it is done,
the card shows the `bootc switch` line to run next.

Without the first line, this machine would take your image whether it is signed or not. Your image
says the same of itself as it is built, so that its updates are checked against your key once the
machine runs it. Then hand the signature policy back to the image, so that later changes to it
arrive with updates rather than staying as they were on the day you switched:

```bash
sudo cp /usr/etc/containers/policy.json /etc/containers/policy.json
```

`ame system rebase` only offers Amethystora's own images, and takes signed ones only. A
[sealed machine](updates.md#sealed-images) cannot switch to an image of your own.
