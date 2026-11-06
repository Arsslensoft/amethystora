# Amethystora Copilot Instructions

This document provides essential information for coding agents working with the Amethystora repository to minimize exploration time and avoid common build failures.

## Repository Overview

**Amethystora** is a cloud-native desktop operating system, based on Universal Blue and maintained by Arsslen Idadi (@iarsslen). It is an OS built on Fedora Linux using container technologies with atomic updates.

- **Type**: Container-based Linux distribution build system
- **Base**: `ghcr.io/ublue-os/silverblue-main` (Fedora with the GNOME desktop and GDM) + Universal Blue infrastructure; GNOME Shell extensions are kept in `system_files/shared/usr/share/gnome-shell/extensions` and set up by `build_files/shared/build-gnome-extensions.sh`: `amethystora-transitions@arsslensoft.github.io` (see The transitions), `amethystora-logo-menu@arsslensoft.github.io` (see The logo menu), `amethystora-clipboard@arsslensoft.github.io` (a modified copy of Clipboard Indicator, see `NOTICE`) and `amethystora-ui@arsslensoft.github.io` (see Amethystora UI); none is a git submodule any more
- **Languages**: Bash scripts, JSON configuration, Python utilities
- **Build System**: Just (command runner), Podman/Docker containers, GitHub Actions
- **Target**: desktop OS with two variants (base + developer experience)

## Repository Structure

### Root Directory Files
- `Containerfile` - Main container build definition (stages: `ctx`, `brew`, `fprintd`, `base`)
- `Containerfile.sealed` - The sealed variant of a built image (`just seal`; see Sealed images)
- `secure-boot.crt` - The certificate of the Secure Boot key sealed images are signed with, once `just secure-boot-key` has made it
- `Justfile` - Build automation recipes (like a Makefile but more readable)
- `.pre-commit-config.yaml` - Pre-commit hooks for basic validation
- `image-versions.yml` - Image version configurations
- `cosign.pub` - Container signing public key
- `keys/` - Upstream's public keys, kept here rather than fetched from a branch that can move: Universal Blue's cosign key (`ublue-os-cosign.pub`, which `just verify-container` checks the base, akmods and brew images with) and the two akmods certificates `just secureboot` checks the kernel against
- `SECURITY.md` - How to report a vulnerability (privately, through the repository's Security tab)

### Key Directories
- `system_files/` - Files copied into the image, overlaid on those of the upstream layers
  - `shared/` - Every image: configurations, fonts, themes, the manual, the `ame` commands
  - `dx/` - `amethystora-dx` only, copied in by `build_files/shared/build-dx.sh`
  - `sealed/` - the sealed images only, copied in by `build_files/sealed/00-sealed.sh`
- `apps/` - The Amethystora apps (Manual, Security, Updates, Notes, Logs, Backups, Control, Help): one Rust
  program, a Cargo workspace built in the Containerfile's `apps` stage (see The Amethystora apps); the same
  workspace builds `amethystora-sensor` and the AI agent's tools (`agent-tools`, see Agentic features); the Vault is
  `apps/vault*`, seven crates (see The Vault)
- `build_files/` - Build scripts organized as base/, dx/, shared/
  - `base/` - Base image build scripts (00-image-info.sh through 20-tests.sh)
  - `dx/` - Developer experience build scripts
  - `sealed/` - The sealing stage of `Containerfile.sealed`: prepare, build the UKI, test
  - `shared/` - Common build utilities and helper scripts
- `branding/` - Source of the artwork; `node branding/generate.mjs` draws it, and `node branding/icons.mjs` the icon packs (see The icon packs)
- `iso/` - `iso.toml`, the bootc-image-builder configuration of the installer ISO; `product/`, which `build-iso.yml` adds to the ISO as `images/product.img`: the installer's Amethystora look (a stylesheet and artwork drawn by `branding/generate.mjs`), crash reporting (libreport files that send crash reports to Amethystora's issue tracker instead of Fedora's Bugzilla) and the Anaconda profile that keeps Fedora's installer settings although the installer's os-release says `generic`; `installer-packages.py`, which lists what the installer is made of; and `live/`, the live system of the live ISO: a `Containerfile` built `FROM` the published image with `build.sh`, which adds Fedora's live boot, live session and Anaconda's web installer, a copy of the image for an offline install (`installer.ks`), a profile for the image's own os-release ID, and the boot menu Titanoboa reads (`iso.yaml`)
- `packaging/` - Packages Amethystora rebuilds, each in its folder with a README: `fprintd/` is Fedora's fprintd with the duress finger's patches, built in the Containerfile's `fprintd` stage on every image build. Renovate looks for a new fprintd release every Monday. The `.patch` files are kept byte for byte (`.gitattributes`, and the pre-commit whitespace hooks skip them)
- `po/` - The gettext catalogs of the shell programs that are translated, the security report and the app permissions (text domain `amethystora`): `amethystora.pot`, which `node build_files/shared/check-translations.js --write --po po system_files/shared` writes from the scripts, and one `<language>.po` each, which `14-security.sh` compiles into `/usr/share/locale`
- `tests/vm/` - The VM lab: a privileged container with QEMU, KVM, Secure Boot firmware, a TPM and the build's
  tools. `run.sh` builds the last commit, installs it onto a disk, boots it and runs `suites/` in it; an MCP
  server (`server.mjs`) drives a VM by hand (`README.md`). Its helper reaches the guest as a systemd credential
  at boot: never put any of it in the image. A new feature gets its check in `suites/` with it. "Run the
  tests" means the `run-tests` skill (`.claude/skills/run-tests`), and only when asked
- `.github/workflows/` - Comprehensive CI/CD pipelines

The desktop layer (the `ujust` recipes, the Homebrew Brewfiles, the setup services and hooks, the
GNOME defaults, udev rules) is kept in `system_files/shared`, already renamed, and is not pulled
from upstream. Only `ublue-os/brew`, for
Homebrew's own setup, is still copied in by the `ctx` stage of the `Containerfile`. Where to edit:
- `system_files/shared/usr/share/amethystora/just/` - the commands `/usr/bin/ame` runs, as just recipes in groups (`ame security scan now`). `00-entry.just` declares each group with `mod` (`agent`, `apps`, `desktop`, `security`, `system`, each in the file of its name), imports the top-level commands (`backup.just`, `changelog.just`, `control.just`, `report.just`, `update.just`), and runs `amethystora-pkg` as `ame pkg`. A command that switches something takes `on|off` (or the value it sets) as an argument, so that Control and scripts can say what they want; without one it may ask or flip, as before. `ujust` is the same command, and the flat names the recipes had under it are private aliases there: when a command is renamed or moved, alias its old name. `20-tests.sh` fails on an `ame` command that the image names and that does not exist, so in prose write `ame` in backticks or with punctuation after it
- `system_files/shared/usr/share/flatpak/preinstall.d/` - the Flatpaks every machine comes with, which `flatpak-preinstall.service` installs on the first boot that has a network (the ISO carries none) and retries until it does: `amethystora.preinstall` is the default list (no browser, Firefox is in the image; Amethystora Logs in place of GNOME Logs), and `system_files/dx` adds `amethystora-dx.preinstall`. Every group needs its `Branch` (`stable`, `3.22` for a GTK 3 theme), and taking an app off a list uninstalls it from the machines that got it from there
- `system_files/shared/usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override` - the base GNOME defaults (the dash, the custom keybinding list); `build-gnome-extensions.sh` adds each extension it installs to its `enabled-extensions`, and `zz1-amethystora-modifications` overrides it

### Architecture
- **Two Build Targets**: `base` (regular users) and `dx` (developer experience)
- **Image Flavors**: main, nvidia-open, nvidia-lts (see Drivers)
- **Fedora Versions**: resolved per build by `just fedora_version` (see Version Detection); `04-packages.sh` has cases for 42 to 45
- **Stream Tags**: `stable`, `latest`, `beta`
- **Build Process**: Sequential shell scripts in build_files/ directory
- **Base Images**: Uses `ghcr.io/ublue-os/silverblue-main` as foundation from Universal Blue

## Build Instructions

### Prerequisites
**ALWAYS install these tools before attempting any builds:**

```bash
# Install Just command runner (REQUIRED for build commands, may not be available)
# If external access is limited, Just commands will not work
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to ~/.local/bin
export PATH="$HOME/.local/bin:$PATH"

# Verify container runtime (usually available)
podman --version || docker --version

# Install pre-commit for validation (usually works)
pip install pre-commit
```

**Note**: In restricted environments, Just command runner may not be installable. Most validation can still be done with pre-commit and manual JSON validation.

### Essential Commands

**Build validation (ALWAYS run before making changes):**
```bash
# 1. Validate syntax and formatting (2-3 minutes)
# Note: .devcontainer.json will fail JSON check due to comments - this is expected
pre-commit run --all-files

# 2. Check Just syntax (requires Just installation)
just check  # Only if Just command runner is available

# 3. Fix formatting issues automatically
just fix    # Only if Just command runner is available
```

**Build commands (use with extreme caution - these take 30+ minutes and require significant resources):**
```bash
# Build base image (30-60 minutes, requires 20GB+ disk space)
just build amethystora latest main

# Build developer variant (45-90 minutes, requires 25GB+ disk space)
just build amethystora-dx latest main

# Build with specific kernel pin
just build amethystora latest main "" "" "" "6.10.10-200.fc40.x86_64"
```

**Utility commands:**
```bash
# Clean build artifacts (if Just available)
just clean

# List all available recipes (if Just available)
just --list

# Validate image/tag/flavor combinations (if Just available)
just validate amethystora latest main
```

**Working without Just (when external access is restricted):**
```bash
# Manual validation instead of 'just check':
find . -name "*.just" -exec echo "Checking {}" \; -exec head -5 {} \;

# Manual cleanup instead of 'just clean':
rm -rf *_build* previous.manifest.json changelog.md output.env

# View Justfile recipes manually:
grep -n "^[a-zA-Z].*:" Justfile | head -20
```

### Critical Build Notes

1. **Container builds require massive resources** (20GB+ disk, 8GB+ RAM, 30+ minute runtime)
2. **Always run `just check` before making changes** - catches syntax errors early
3. **Pre-commit hooks are mandatory** - run `pre-commit run --all-files` to validate changes
4. **Never run full builds in CI unless specifically testing container changes**
5. **Use `just clean` to reset build state if encountering issues**

### Common Build Failures & Workarounds

**Pre-commit failures:**
```bash
# Known issue: .devcontainer.json contains comments (invalid for JSON checker)
# This failure is expected and can be ignored:
# ".devcontainer.json: Failed to json decode"

# Fix end-of-file and trailing whitespace automatically
pre-commit run --all-files
```

**Just syntax errors (if Just is available):**
```bash
# Auto-fix formatting
just fix

# Manual validation
just check
```

**Container build failures:**
- Ensure adequate disk space (25GB+ free)
- Clean previous builds: `just clean` (if available)
- Check container runtime: `podman system info` or `docker system info`
- Build failures often indicate resource constraints rather than code issues

## Validation Pipeline

### Pre-commit Hooks (REQUIRED)
The repository uses mandatory pre-commit validation:
- `check-json` - Validates JSON syntax
- `check-toml` - Validates TOML syntax
- `check-yaml` - Validates YAML syntax
- `end-of-file-fixer` - Ensures files end with newline
- `trailing-whitespace` - Removes trailing whitespace

**Always run:** `pre-commit run --all-files` before committing changes.

### GitHub Actions Workflows
- `build-image-latest-main.yml` - Builds latest images on main branch changes
- `build-image-stable.yml` - Builds stable release images
- `build-image-beta.yml` - Builds beta images
- `build-images.yml` - Runs the stable, latest and beta builds together (manual dispatch only)
- `reusable-build.yml` - Core build logic for all image variants, and on the stable stream the sealed images (see Sealed images)
- `generate-release.yml` - Generates release artifacts and changelogs
- `build-iso.yml` - Builds the installer ISO of a stable image, `<image>-non-uefi.iso` (the only one that starts computers without UEFI), with bootc-image-builder and uploads it, with its checksum signed by cosign, to Cloudflare R2 (a Release asset is limited to 2 GiB). Both of its ISOs are built from one digest of the published image, resolved and checked against `cosign.pub` once (the `image` job), so that the image cannot change between the check and either build. The builders run privileged and hold no secret: each hands its ISO, as an artifact, to the `publish` job on a runner of its own, which works out the checksum again, signs and uploads, and only on main. `Stable Images` runs it after scheduled and dispatched builds for each of the six images, each upload replacing the last under the same name, and the workflow fails on any other image. bootc-image-builder picks the installer's packages by os-release `ID` and has no definition for `amethystora`, so the workflow runs its container by hand and writes Fedora's definition out under that name, with `generic-logos` and `generic-release` added so that the installer carries neither `fedora-logos` nor `fedora-release`; do not switch back to its GitHub action, which cannot. `iso/installer-packages.py` then lists the installer's packages from the builder's manifest for the ISO to carry beside `LICENSE` and `NOTICE`, and fails the workflow before the upload if one of Fedora's two is among them. Both kickstarts (`iso/iso.toml`, `iso/live/installer.ks`) switch the installed machine with `--enforce-container-sigpolicy`, which only rewrites the origin and needs no network, and both ISO builds fail on a switch without it. Their `%post` runs under `set -e`, since Anaconda goes by its last exit status only, and on an NVIDIA image with Secure Boot on queues the image's certificates for the MOK manager (password `amethystora`), since the kernel refuses NVIDIA's driver until its key is enrolled and the image's kernel arguments turn off the display driver that would stand in. The builder is pinned by digest to its last build that reads definitions from a folder; its successor compiles them in, so moving to it means rewriting that step. Its second job, `live-iso`, builds the live ISO of the image, `<image>.iso` (`amethystora.iso` is https://download.amethystora.org/amethystora.iso): it builds `iso/live` `FROM` the checked digest with `podman build --cap-add sys_admin` (`iso/live/build.sh` pulls the offline copy by the same digest), and has Titanoboa (`ublue-os/titanoboa`, Apache-2.0, pinned to a commit of its main branch from the first that takes only an image) squash it into a UEFI-only ISO, then lists the live system's packages, fails on Fedora's logo or release package, adds `LICENSE`, `NOTICE` and the list, and hands it to `publish` as the installer job does. Titanoboa's offline-install store in `/usr/lib/containers/storage` does not work with Fedora 45's podman 6 (ublue-os/titanoboa#151): solve that before the stable image moves to 45
- `bump-base-images.yml` - Daily on main: resolves every tag in `image-versions.yml` to its digest, checks each against `keys/ublue-os-cosign.pub`, and proposes the changed digests in one pull request from `automation/base-images`. Its pull requests start their checks only with a `BASE_BUMP_TOKEN` secret (a token with Contents and Pull requests write on this repository); `20-tests.sh` warns when the base is more than 10 days older than the build
- `security-rebuild.yml` - Daily on main, read-only but for the dispatch: compares the published stable image's source packages (its SBOM) with Fedora's security updates in Bodhi, and starts `Stable Images` when one was pushed after the published image's base was built and before the pinned base was. The kernel is left to the weekly build. Images carry `org.amethystora.base-created` for it
- `checks.yml` - The quick checks the `main` ruleset requires of a pull request and a merge group: `Lint` (pre-commit and ShellCheck on the files the change touches, `just check`, `check-translations.js` and `check-manual.js` as `20-tests.sh` runs them, actionlint), `Title` (a Conventional Commits title, which squash-merging makes the commit's) and `Dependencies` (`dependency-review-action`). Renaming a job means changing the ruleset with it. CodeQL's default setup scans the code and the workflows, and CodeRabbit comments on each pull request (`.coderabbit.yaml`); neither decides a merge
- `validate-renovate.yml` - Validates the Renovate configuration on pull requests
- `check-windows-programs.yml` - Weekly, and on a change to the map: every Flatpak ID in `windows-programs.json` is still on Flathub (see Windows programs)
- `scorecard.yml` - OpenSSF Scorecard supply-chain checks (weekly)
- `clean.yml` - Cleanup old images and artifacts
- `moderator.yml` - Repository moderation tasks

**Workflow Architecture:**

- Stream-specific workflows (stable, latest, beta) call `reusable-build.yml`
- `reusable-build.yml` builds both base and dx variants for all flavors (main, nvidia-open, nvidia-lts)
- Fedora version is dynamically detected based on stream tag
- Images are signed with cosign and pushed to GHCR, and every tag pushed is checked against `cosign.pub` afterwards
- Only main publishes: one `PUBLISH` condition in `reusable-build.yml` (main's schedule and dispatches, the merge queue in front of main, and the `beta` branch's own stream) decides every push, signature, attestation and SBOM, and which builds get the real module and Secure Boot keys. Everything else builds with throwaway keys and publishes nothing. No build step gets `GITHUB_TOKEN`, and keys reach the build as files (`AKMODS_PRIVKEY_FILE`, `SECUREBOOT_KEY_FILE`), never on a command line. The keys and the R2 token are secrets of the `release` environment alone, which admits `main`, `beta` and `gh-readonly-queue/main/*`: the image job names it under the same condition as `PUBLISH`, and the ISO's `publish` job always, so a branch that edits either workflow still gets no key. A new job that needs one names the environment too; never add a key as a repository secret
- cosign is installed from its pinned installer before the build, into `/usr/local/bin` for sudo, and `just verify-container` fails without it; just comes from a pinned release with its checksum (`.github/actions/install-just`)
- Every workflow sets `permissions: {}` or read-only access at the top and grants write access on the job that needs it, which OpenSSF Scorecard checks (`build-images.yml` is the exception: each job it runs needs the same write access)

### Manual Validation Steps
1. `pre-commit run --all-files` - Runs validation hooks (2-3 minutes, .devcontainer.json failure is expected)
2. `just check` - Validates Just syntax (if Just is available, 30 seconds)
3. `just fix` - Auto-fixes formatting issues (if Just is available, 30 seconds)
4. Test builds only if making container-related changes (30+ minutes)

## Package Management

### Package Configuration
Packages are defined directly in build scripts rather than in a central configuration file:
- `build_files/base/04-packages.sh` - Core package installations
  - `FEDORA_PACKAGES` array - Packages from official Fedora repos (installed in bulk)
  - `COPR_PACKAGES` array - Packages from COPR repos (installed individually with isolated enablement)
  - Fedora version-specific package sections using case statements (e.g., `42)`, `43)`)
- `build_files/dx/00-dx.sh` - Developer experience package additions. The editor is VSCodium, from its repository with the key pinned in `system_files/dx/etc/pki/rpm-gpg`: Microsoft's VS Code licence does not allow it to be shared inside a published image, so do not bring it back. `user-setup.hooks.d/10-vscodium.sh` installs only Open VSX extensions their makers published there, not copies its bot republished. Nothing joins an account to `docker`, `incus-admin` or `libvirt`, each root under another name: `ame system dx-group` offers the groups listed in `system_files/shared/usr/share/amethystora/developer-groups`, which Control and the security report read too

### Games, Android apps, drawing tablets and the CPU scheduler

Each is installed and off, and turned on per machine or per account by one command that says first what
it changes. `ame apps gaming` installs Steam, ProtonPlus and the MangoHud and gamescope Vulkan layers of
Steam's own runtime branch from Flathub, and grants X11 back to Steam alone, for the account
(`flatpak override --user`), and gives `/dev/uinput` back for Steam Input (`ame security virtual-input off`;
the image keeps it root's, Solaar's rule included, since a virtual keyboard types into any window, and
`ame apps opentabletdriver` does the same); Steam itself never goes into the image (its package is RPM Fusion's
non-free one). `gamemode` is in the image for games to reach through its portal. `ame apps android`
sets up Waydroid (Fedora's package, with its own SELinux policy; binder is built into Fedora's kernel):
it enables `waydroid-container.service`, which ships disabled, and downloads LineageOS without Google's
apps on the machine; never offer Google's. `ame system scheduler` picks a sched-ext scheduler through
`scx_loader.service` (shipped disabled, configured in `/etc/scx_loader/config.toml`) from CachyOS's
COPR `bieszczaders/kernel-cachyos-addons`, which Bazzite uses too: Fedora's own scx packages predate the
kernel interface. It checks `/sys/kernel/sched_ext` and falls back to the kernel's scheduler when the
kernel will not load one. `ame apps opentabletdriver` installs OpenTabletDriver from Flathub and copies
the udev rules of its release into `/etc`: the tag and the SHA-256 of the tarball are pinned in
`apps.just`, where Renovate's `github-release-attachments` manager bumps both, so nothing reaches `/etc`
from an unchecked download. Its daemon runs from the image's own user unit,
`amethystora-opentabletdriver.service`, which nothing enables for every account.

### Drivers

`plans/done/PLAN-driver-compatibility.md`. The rest of Universal Blue's common kmods come from the
`akmods` tarball `03-install-kernel-akmods.sh` already downloads, built for the image's kernel and signed
with Universal Blue's key, which `ame security secure-boot` enrolls; `20-tests.sh` fails on a module under
`extra` (evdi aside) signed with a certificate the image does not ship.
- **NVIDIA.** `-nvidia-open` carries NVIDIA's open driver (Turing and newer, PCI device ID 0x1E00 and up),
  `-nvidia-lts` its closed driver of the 580 branch (`akmods-nvidia-lts`; Maxwell, Pascal and Volta, 0x1340
  to 0x1DFF), the last NVIDIA makes for them, so that flavor has an end date; nothing NVIDIA makes drives older
  cards. Both bounds live in `amethystora-gpu-check` alone (`offer` prints its choice, which the driver
  check and the live ISO's sealed installer ask). `image-info.json`'s `image-flavor` is the flavor that ends
  the image's name. `amethystora-rollback-helper` refuses an NVIDIA image while Secure Boot is on and
  `akmods-amethystora.der` is not enrolled, which would start to a black screen. The `-nvidia-lts` images have
  their ISOs like the others; their GPL user space comes from negativo17's `nvidia-580`, which `source-manifest.sh` knows.
- **Xbox controllers.** xpadneo (Bluetooth) and xone (Microsoft's wireless adapter; wired pads are the
  kernel's `xpad`). `xone-kmod-common` carries Microsoft's adapter firmware, which no licence lets the
  image pass on, under names the driver no longer asks for: the build deletes it and moves its `blacklist
  mt76x2u` (which takes Wi-Fi from MT7612U sticks) to `/usr/share/amethystora/xone`, and
  `source-manifest.sh` never fetches its source package (`NOT_FETCHED`). `ame apps xbox-adapter on`
  downloads Microsoft's four driver packages, checks them and their firmware against
  `/usr/share/amethystora/xone/firmware` (static since 2017, so Renovate does not follow them), installs
  `xone_dongle_<ID>.bin` into `/var/lib/amethystora/firmware` (tmpfiles, root's), copies the blacklist and
  appends `firmware_class.path` with `rpm-ostree kargs`, never in `kargs.d`; sealed images refuse it. The
  kernel reads that folder before the image's firmware, so the watcher's drift check takes no file there but
  the pinned ones and no other firmware path.
- **Left out:** Broadcom's `wl` (its licence makes whoever passes it on indemnify Broadcom, and its
  CVE-2019-9501 and 9502 can never be fixed) and openrazer (its daemon is Terra's). `20-tests.sh` fails on
  either package, or on a blacklist of the kernel's own Broadcom drivers.
- **The driver check** (`amethystora-driver-status`, `amethystora-driver-check.timer` as root) saves
  `/var/lib/amethystora/drivers/status.json` for `ame system drivers`, `ame report` and the doctor, and says each
  finding once (`said` beside it): a PCI device of a kind a person notices with no driver, firmware asked for
  by a device left without one, a refused module, NVIDIA's driver not loaded, a Broadcom Wi-Fi card, the
  adapter without its firmware, a DisplayLink dock (USB vendor `17e9`) whose evdi did not load. That one is also
  started when the dock arrives (`99-amethystora-displaylink.rules`, after the service that tries to load it), its
  state is the finding's id (`displaylink:needs-enrollment`, `enrollment-pending`, `lockdown`,
  `sealed-no-secure-boot`, `service`, `refused`), said again only when the state changes, and it offers enrolling
  only with Secure Boot on, since without it lockdown refuses every key but the kernel's (`test-driver-status.sh`).
- **FireWire** is a switch of its own (`ame security firewire`), its modules in
  `usr/lib/modprobe.d/61-amethystora-firewire.conf`, apart from the blocked modules.
- **Lockdown** stays as the gaps plan left it (the maintainer's choice of 2026-10-07, over this plan's of
  2026-10-04): the argument on every image but the NVIDIA ones, and `ame security lockdown`.
- Kernel modules' source packages are `kmod-NAME`, built by Universal Blue from the akmod their makers publish
  as `NAME-kmod`: `source-manifest.sh` fetches that one and links the kmod's name to it.

### The browser

The browser is Firefox, Fedora's package, installed by `04-packages.sh` so that a machine has one
before it is online. Mozilla lets Firefox be redistributed only unaltered, so it ships exactly as
Fedora builds it: do not add a `policies.json`, an extension, a preference file or a home page for
it, in the image or from a setup hook (`20-tests.sh` fails on a policy file). That is why the image
has no browser policy and installs no extension. The one thing taken out, by the maintainer's choice
of 2026-10-04, is Fedora's start page: `04-packages.sh` deletes the two lines of Fedora's preferences
that open Firefox on start.fedoraproject.org and pin it on the new tab page, which leaves Firefox's
own home page. Never put amethystora.org or any other page there: Mozilla's distribution policy
names a changed home page among the changes that need its written consent. Brave, the browser before it, is not in the image
and not on the Flatpak lists: its terms license its package for personal use, not for
redistribution. Like every other browser, it is the user's to install from Bazaar.

Browser protection, the extension allowlist and the block on WebUSB, WebSerial, WebHID and Web
Bluetooth, is opt-in for the same reason: a policy the machine's owner turns on is theirs, not the
image's. `ame security browser` runs `/usr/libexec/amethystora-browser-policy`, which
writes `usr/share/amethystora/browser-policy/chromium.json` or `firefox.json`, with the IDs in
`/etc/amethystora/browser-extensions` added, to every place in its `TARGETS` table: Brave, Chrome,
Chromium, Edge and Firefox, as packages and as Flatpaks. To cover another browser, add its line
there. Keep the Firefox forks out: a system policy replaces the one they ship. The policies only
allow extensions and must never install one. The Security report reads the helper's `status`. Site
checking (`ame security browser osprey`, `OSPREY` in `security.conf`) adds Osprey's three IDs to the
allowlists while chosen, after its explanation: it sends every site's address to Osprey Project LLC, so it
is offered only, in no profile, never in the built-in list and never installed, and Osprey's own settings
are left to the user.
**Data collection off** (`ame security browser privacy`, the marker `/etc/amethystora/browser-privacy`, no key in `security.conf`) is a second,
independent part of the same helper (`plans/done/PLAN-browser-telemetry.md`): Firefox's `DisableTelemetry`, `DisableFirefoxStudies`, and no sponsored
shortcuts, stories or suggestions (`firefox-privacy.json`), and each Chromium-family maker's own reporting keys (`chromium-privacy.json`, its own
`11-amethystora-privacy.json` beside protection's file; Firefox's single `policies.json` is the two merged by one write function, which
`build_files/shared/test-browser-policy.sh` runs against a stand-in root). It is off by default and in no profile, it never installs an extension or touches
a home page, and it is the owner's command, never the image's: keep it so.

### The terminal and the prompt

The terminal is kitty (the maintainer's choice of 2026-10-24, over Ptyxis, which the image had used since the
Hyprland era's kitty went): Fedora's package, first in `xdg-terminal-exec`'s three lists (`/etc/xdg/xdg-terminals.list`,
`gnome-xdg-terminals.list`, `usr/share/xdg-terminal-exec/gnome-xdg-terminals.list`: 20-tests.sh holds all three), so
`Super+Return`, `amethystora-in-terminal`, the logo menu and every `Terminal=true` launcher open it; its launcher's
`X-TerminalArg*` keys say how. Ptyxis, the base image's, stays installed and second, for its container menu. There is one
launcher (the maintainer's choice of 2026-10-30): `17-cleanup.sh` gives kitty's the name, translations and icon of a Terminal
and makes Ptyxis's `NoDisplay`, never `Hidden`, which xdg-terminal-exec would drop, and hides `kitty-open.desktop`, which
listed a kitty icon under Open With. Files' Open in Console asks only for `org.gnome.Ptyxis` on the bus, so that name's service starts
`/usr/libexec/amethystora-terminal-here`, which opens the default terminal in the folder and exits, leaving the name to `ptyxis`.
- **Settings are the image's `/etc/xdg/kitty/kitty.conf`**, which kitty reads under the account's own
  `~/.config/kitty/kitty.conf` (a line of the account's wins; checked against kitty 0.47). Keep the title bar
  (`hide_window_decorations no`: the Hyprland era's config hid it and left GNOME windows without a close button), keep
  `update_check_interval 0` and `allow_remote_control no`.
- **Its colours are the theme's**: `themed/kitty-theme.conf.tpl`, rendered into `~/.config/amethystora/current/theme`
  and included by the image's config from there; `apply_kitty` in `amethystora-theme` sends SIGUSR1 to the account's
  own open kitty windows so that they take a new theme at once. `test-theme-render.sh` has kitty and starship load
  every shipped theme's rendering. The Ptyxis palette is still rendered, for the second choice.
- **The prompt is Starship as a powerline**, `themed/starship.toml.tpl`: user@host, the folder and, in a git
  repository, the branch, each on a background of the theme's colours (`surface`, `accent`, `repo` in its palette,
  text in the theme's `background`), the arrows Nerd Fonts' U+E0B0, then the command on a line of its own after `▶`.
  The folder's `repo_root_format` carries the arrow into the branch and the branch's `( )` group ends the line, so
  an arrow never points at nothing; keep both when changing it. Write glyphs as `\uXXXX` escapes: the private-use
  ones do not survive an editor or a tool. `tests/vm/suites/97-terminal.sh` is its check.

### Agentic features

Every image, base and dx, ships `amethystora-agent`, the launcher behind `Super+Ctrl+Shift+A` and
the menu's "Ask an agent"; `ame agent` alone opens it too (the group's `[default]` recipe, so `agent.just` has no
`default-list`), and `ame agent ask|diagnose|list|status|search|read|state` pass to it, the first three on the never list. The agents it offers, Claude Code, Opencode, Codex, Antigravity CLI (`agy`),
Copilot CLI and Cursor CLI (`AGENTS` in the launcher, which `amethystora-agent list` prints for Control), are
deliberately not in the image: they release far more often than it does, so the launcher installs the
chosen one into the user's home with its maker's installer on first launch (or `ame agent install`),
where it updates on the user's terms. Antigravity CLI replaced Gemini CLI, which came from Homebrew: a
Gemini CLI already chosen and installed still opens, and the security readers still know it. Do not add them to `04-packages.sh`; `20-tests.sh` fails if
one appears in `/usr/bin`. The
skill in `system_files/shared/usr/share/amethystora/agents/skills/amethystora/SKILL.md` tells
agents how to change *a user's machine* safely: this repository is where it is written, not a place
it applies to. `amethystora-diagnose/SKILL.md` beside it is the read-only method behind
`amethystora-agent diagnose`. `user-setup.hooks.d/13-agentic.sh` links every skill there into
`~/.claude/skills`, which Claude Code reads, `~/.agents/skills`, which the others read, and
`~/.gemini/antigravity-cli/skills`, Antigravity CLI's own. Opencode reads
both and requires skill names to be unique across them, so the launcher starts it with
`OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1`. The features are **off by default** (the maintainer's choice of
2026-10-18): `ame agent toggle on` turns them on per user by writing `~/.config/amethystora/agentic`, and
the image ships no account with it. The update that made them so left every account off;
`user-setup.hooks.d/13-agentic-off.sh`, which runs before `13-agentic.sh` unlinks the skills it looks for,
takes away the old `no-agentic` marker and tells an account that had them on, once. Keep the skill
accurate when you change a command, path or recipe that it names. The main skill is a short core (under
7 KB, `20-tests.sh`) and `references/<topic>.md` files it names, read only when needed.

**Two levels, and the tools** (`plans/done/PLAN-agentic-os.md`). Level 1, "Agentic features", is the agent, its
skills and Amethystora's own read-only tools; level 2, "Let the agent make changes" (`ame agent tools on`,
the marker `~/.config/amethystora/agent-tools`, which needs level 1), adds the tools that change things.
The tools are one Rust program, `apps/agent-tools` (`/usr/libexec/amethystora-agent-tools`, no GTK): an MCP
server over stdio written by hand, which the agent starts and which lives only while it does, and the same
answers as a command (`amethystora-agent search|read|state|history`). Level 1 is `search` and `read` (the
index), `state` (thirteen topics, each a projection of the helper that already answers it, never state worked
out on its own), `logs` (every value given to journalctl as its own option) and `history`, pre-allowed in
the agents. Level 2 is `change` (a Control entry, through `apps/control-core`, the GTK-free crate Control
uses too), `software` (Flatpak, Homebrew, amepkg; never layering), `run` (a command handed to the user, in a
terminal or as root) and `undo`, each asked for every time; every change is recorded in
`~/.local/state/amethystora/agent-tools/actions.jsonl` with its way back. Text written by others reaches the
agent cleaned and marked as data. Off costs nothing: no daemon, timer or service.
- **The launcher gives each agent the tools at launch** and writes nothing into its settings: Claude Code
  `--mcp-config` and `--allowedTools`, Copilot CLI `--additional-mcp-config` and `--allow-tool`, Codex `-c
  mcp_servers.amethystora...` with each read tool's `approval_mode`, Opencode `OPENCODE_CONFIG_CONTENT`. Cursor
  CLI and Antigravity CLI take no such option, so while the features are on their settings hold exactly
  one entry (`~/.cursor/mcp.json`, `~/.gemini/config/mcp_config.json`, and the read tools' allow rules),
  which the security reader accepts and nothing more under that name. `amethystora-agent argv AGENT` prints
  a launch, and `20-tests.sh` holds every option to `agent-cli-flags`. Check a new option with the installed
  agent's `--help` first, as these were on 2026-10-19.
- **The never list** (`apps/agent-tools/src/never.rs`) is refused by the tools and again by the root helper:
  the duress feature, a security command that loosens protection or sends something away (only reads and
  turning protection on pass), deleting from a backup or a snapshot, erasing disks, the AUR, sudoers, polkit,
  PAM, the agent's own lock and root mode, the update boot guard and the lost-computer line, the Notes data, the Vault, and shells or wrappers that would run commands out
  of sight. The tools that change things refuse while the agent runs with its own checks off (a flag from
  `agent-cli-flags` on the agent above the server, or a setting the security reader reports).
- **`/etc/amethystora/agent.conf`** is the administrator's: `ALLOW=act|know|off` (the most any account may
  turn on), `ALLOW_FOR=account:level` (one account's own), `ROOT=session|minutes|each|terminal`. Its one
  reader and writer is the tools program (`policy`, and `conf` as root, behind `ame agent lock` and `ame
  agent root`); a value it cannot read counts as the strictest. The watcher's drift check accepts it.
- **Root commands** run through `/usr/libexec/amethystora-agent-root`, which pkexec starts from the tool
  server with a socket pair on its stdin and stdout (a pipe could be opened by another process of the
  account through `/proc/PID/fd`), one polkit action per mode in `org.amethystora.agent.policy` (picked by
  `exec.argv1`), no rule letting anyone through. It reads agent.conf itself, runs only for the tool server of
  an account allowed to act, `env -i`, ten minutes at most, and says every command in a notification and the
  journal (`amethystora-agent-root`). In `session` it serves until the agent ends, the screen locks, an hour
  passes or agent.conf changes. `amethystora-in-terminal --agent --report` is the hand-off to a terminal.
- **Control's AI agent page** shows the root mode and the lock, each an `ame agent` command in a terminal:
  the maintainer's exception of 2026-10-18 to Security keeping every security setting, since they govern
  the agent, not a security feature.

**The knowledge and the index.** `system_files/shared/usr/share/amethystora/agents/knowledge` is written for
agents: `architecture.md`, `by-design.md` (each default the image hardens, its symptom, how to confirm it and
its switch), `desktop.md`, `agent.md`, `playbooks/<symptom>.md` (Symptom, Look, Causes on Amethystora, Fix,
Verify, Undo; `amethystora-doctor` findings name them) and `aliases.tsv`. **Keep it accurate** like the manual
and the skill: `20-tests.sh` fails on a `/usr` path, unit or `ame` command it names that the image lacks, and
nothing in it may name the duress feature. `build_files/shared/build-agent-index.sh` writes the index when
the image is built: the manual, the skills, the knowledge and a reference generated from the image itself
(every `ame` command from just's dump, helper header, unit, security switch and Control entry), cut into
passages of about 300 tokens whose ids are the manual's links, ranked by BM25F and by meaning with
potion-base-8M (Model2Vec, MIT), whose tokenizer and safetensors reader are in the crate and are held to the
model's own output (`tests/model-reference.jsonl`). `apps/agent-tools/tests/golden.tsv` is asked of the
source tree's index by cargo and of the image's by `20-tests.sh`, at recall@3 of at least 0.9: add a question
when you add a page an agent should find. `/usr/libexec/amethystora-doctor` (`ame system doctor`, `state
health`) is the read-only health check outside security, about ten seconds, no password.

Updates, Security and Logs offer **Ask the agent** where something failed, while the features are on
(`amethystora-agent status`): each runs `amethystora-agent diagnose` with what the program picked
itself, never words shown in its window. Updates passes the failed updater unit (one of its two), Security the
check's id looked up in the report it read (and passes that check's title and sentence), Logs a crash's
process number or the `.service` an entry came from, both checked for their shape.

**On the desktop** (`modules/agent.js`, `modules/quit.js`, `plans/beyond/spec-r4-agent.md`), only while the features are on:
`Super+Shift+A` hands the primary selection (read through Mutter, 16 KiB at most) to `amethystora-agent ask --quoted
[--question Q] -- TEXT`, which keeps 4000 characters and bars every line as data (NEL, U+2028 and U+2029 count as line
ends; `build_files/shared/test-agent-text.sh`). While level 2 is on, the live pill's `agent` activity lists what the tools
record (`actions.jsonl`, and the running tool servers each named in `$XDG_RUNTIME_DIR/amethystora/agent-sessions/PID`, which
`amethystora-agent-tools mcp` writes and takes away at its end and on SIGTERM, SIGHUP or SIGINT), each change with Undo
(`amethystora-agent undo ID`, the tools' own undo, which refuses a record whose way back is not the inverse of what it says
it did) and Stop (`amethystora-agent stop`: SIGTERM to the account's tool servers, not a hard stop). Force quit
(`Super+Ctrl+Shift+Escape`) ends an app only with Mutter's `window.kill()`, as GNOME does; GNOME's not-responding question
gains Ask the agent why (`diagnose --frozen PID`). The image's failure notices offer Ask the agent only to an account whose
features are on: `amethystora-notify-users --agent-unit UNIT` (the scan alert) raises the notice in that account's session
through `amethystora-agent-notice`, so that the root unit never waits; `--agent KEY=LABEL` (the driver check). Control's AI
agent page has both desktop switches (`ame agent selection`, `ame agent pill`). The never list refuses any spelling of the
agent's desktop keys, `ame agent pill [off]`, `ame desktop pill [off]` and `gnome-extensions` on Amethystora UI. Before it
matches, `never.rs` reads an `ame`, `ujust` or `just` command as the recipe's words: options and their values left out,
every `--` passed over, `a::b` as two words, an option that runs something other than a recipe (`--command`, `--set`,
`--evaluate`...) refused, and a group's own justfile named with `-f` or `--justfile` (`just -f .../agent.just pill off`)
matched both as given and with the group in front.

**The agent in the pill** (round 7, `plans/ui-ideas/r7/spec-agent.md`, `modules/agent/{ask,askmodel,doingmodel,routines,routinesmodel}.js`; keys of the
agent's schema, none in the setup file but `ask-here`). **What it reads** (`doing`, on): the tool server writes the fixed name of the read tool it runs (and
`state`'s topic from its list) into its own session file in `$XDG_RUNTIME_DIR/amethystora/agent-sessions/PID` (`history.rs` `working`/`worked`), never an
argument, and the pill says it with a tick. **Ask right there** (`ask-here`, `Super+/`: `Super+Shift+Space` is GNOME's next layout): `amethystora-agent
ask-here` takes `{question, quoted?, picture?}` on standard input (never a command line) and runs the agent without a terminal and without any tool but the five
that read: `HEADLESS` in the launcher names the agents whose options were read from their own `--help` (Claude Code, 2026-11-06; the others open in a terminal),
`AMETHYSTORA_READ_ONLY=1` makes the tool server drop the tools that change things (it can only lower the level), and its answer is shown as text, kept ten minutes,
never written. **Yes or no** (`ask-questions`, on): a hand-off (a `run`, a terminal `change`) is put to the person over the session bus,
`org.amethystora.Agent.Ask(id)` on GNOME Shell, which answers with their click; the question is `ask.json` in the hand-off's own folder, written by the tools, and
**no file stands for a yes**, so nothing an agent can write answers for the person. After an Allow the terminal opens with `--approved`; with no live pill, a command too long
to read in a card, or the key off, the terminal asks as it always did. A password for root is sudo's, and the root helper's modes are not touched. **Ask about this**
(`ask-about`, offered, off, and never in the setup file): a picture or text file dropped on the pill's Ask target or a copied picture with `Super+Shift+A`, shown with where it
goes before it goes, copied into `$XDG_RUNTIME_DIR/amethystora/ask` (0700) only on Send and taken away after, refused while the screen is shared. **Routines**
(`routines`, `apps/agent-tools/src/routines.rs`, `ame agent routines`): one JSON file with one trigger and up to eight actions, each an app's launcher id or `ame` words, never a
shell line, root, `ame security|vault|agent|plugin`, anything `never.rs` refuses or a literal secret; sealed under the Vault's `background` key (record `routines`, the one purpose served
with the vault closed) in `~/.local/share/amethystora/routines/routines.avs`; the agent writes and `check`s, only the person `add`s (in a terminal, no `--yes`), `resume`s,
`run`s and removes (`never.rs`). `ame` words are resolved with `ame --show` (a one-word alias stands for its recipe) before the refusals, at `check` and at every `fire`; an `open` action runs
with its output nowhere and never waits for the app. With no sealed file there is no folder, no watch and no call to any vault: the routines start only while the agentic features are on, look for
the file, and are told by `add`/`pause`/`resume`/`remove` over `org.amethystora.Agent.RoutinesChanged`; the shell connects only the sources the routines name. They run only while the Vault is open unless its
background key is on (a lock closes it: the routines are looked for again while its window is open; a run that meets it closed is missed and said once). A lock stops the extension, so nothing runs while
the screen is locked but a routine for the lock itself, which must say `while_locked`; the unlock event is fired once the routines are read. The hand-off's `Ask` carries the SHA-256 of `ask.json` and is accepted only
from a child of the tool server named in it; a question nobody answered (lock, features off, the hour) is `org.amethystora.Agent.Unanswered`, recorded `not run`, never a no.

### The Amethystora apps

The Manual, Security, Updates, Notes, Logs, Backups, Control and Help are one program, written in Rust with
GTK 4 and libadwaita: the Cargo workspace in `apps/`, one crate for each app beside `common` (the
crate `amethystora-ui`, which they share) and `launcher`, which picks the app by the name it was started
as. The Containerfile's `apps` stage builds it on the same pinned base with Fedora's Rust
(`build_files/shared/build-apps.sh`), once every crate's tests pass, and `13-apps.sh` hard-links it to
each app's own path, `/usr/lib/amethystora-<app>/amethystora-<app>`, which the `/usr/bin` launchers
start. They were Electron windows of some 300 MB each until October 2026, and look as those did: keep
the look when you change one.

- `common` holds what the windows share: the frame (sidebar and window controls), the palette, which
  follows `~/.config/amethystora/current` as the theme changes, the wordmark (drawn live in Quicksand,
  from `apps/common/assets/gem.svg` and `gem-stem.svg`, which `branding/generate.mjs` writes), the
  icons, Markdown (`markdown.rs`, raw HTML shown as text), translations and running programs
  (`process.rs`), always as an argument vector, never through a shell. Each app's stylesheet is beside
  its code; the desktop's own `gtk.css` is kept out of the windows.
- A label with links in its markup goes through `ui::link_accessibles` (`Rich` and `ui::markup` call it). If it does not, GTK 4.22
  registers the label twice with the accessibility bus, and the app aborts under a screen reader after a page change.
- The same program is the Phone (`apps/phone`, see The phone link) and the Shelf (`apps/shelf`, `/usr/bin/amethystora-shelf`,
  see Amethystora UI's `shelf`), which `13-apps.sh` installs like the rest. An app may read what its launch hands it on
  standard input (`App::command_line`), which the shelf's first drop uses: a drop never travels on a command line.
- A window's app id is `org.amethystora.<App>`, which its launcher's `StartupWMClass` names, so that GNOME
  finds the launcher's name and icon for it.
- An app keeps how it was left in `~/.local/state/amethystora/<app>.json`.
  `user-setup.hooks.d/20-retire-electron-profiles.sh` takes away the Chromium profiles the Electron
  windows kept in `~/.config/amethystora-<app>`.
- `apps/Cargo.lock` pins every crate by checksum. Renovate's cargo manager bumps them, gtk-rs and ICU4X
  each as one group. The licences of the crates built into the program ship in
  `/usr/share/licenses/amethystora-apps/crates`, beside `CRATES`, their list, and `build-apps.sh` fails
  on a crate without a licence file of its own.
- The apps stage has Node as well, which runs `20-tests.sh`'s checks written in JavaScript
  (`check-manual.js`, `check-translations.js`) from where the stage is mounted, `/apps-stage`, and
  Notes' compatibility tests: the image carries no Node.
- The VM lab drives a window as a person would, through its accessibility tree
  (`tests/vm/suites/atspi.py`).

### The manual

The Amethystora Manual (`amethystora-manual`, `Super+F1`, "Manual" in the Amethystora menu) is the
user documentation, shipped in the image so that it always describes the image it is on. Its pages
are the Markdown files in `system_files/shared/usr/share/amethystora/manual`, listed and grouped in
`pages.json`; the hotkeys page is `keybindings.md` one level up, which `ame desktop keybindings` also
shows. **Keep the manual accurate** when you change a command, recipe, hotkey, path or default that
it describes, and add to it when you add something a user would look for there.

- Link between pages as files (`updates.md#rolling-back`). `build_files/shared/check-manual.js`
  runs in `20-tests.sh` and fails the build on a page that is missing or a link that lands nowhere.
- `07-debrand.sh` rewrites every Universal Blue name in the image except on a line that
  also carries a copyright or licence phrase, or a comment that opens with a credit (`# Written by`,
  `# Modified by`, `# Source:`), so upstream is credited only that way (`about.md`).
- The window is `apps/manual`, one of the Amethystora apps. It only shows the manual's own pages, opens
  web links in the browser, and follows the theme's palette. The sidebar draws the same wordmark as
  Security.
- A page translated into another language is `manual/<language>/<page>.md` (the hotkeys page too, as
  `manual/<language>/keybindings.md`). Weblate writes them from the English, paragraph by paragraph, so
  edit the English only. A translated page's headings take the ids of the English page's headings in
  the same order, so `updates.md#rolling-back` lands in every language; `check-manual.js` checks the
  translations' links against the English headings and warns when a translation has a different number
  of headings. Until Weblate brings such a page back in line, a link to a section it cannot find opens the
  English page at that section, with a note over it and the way back. A page a language has not translated is
  still named in that language in the sidebar, from the Manual's catalogs (`check-translations.js`'s `DATA`), and
  reads left to right in Arabic.
- `whats-new.md` is the build's, not written by hand: `just build` runs `build_files/shared/release-commits.sh` on the host,
  which reads the published image's `org.amethystora.whats-new` label and the `feat` subjects since its commit into
  `build_files/whats-new.json` (gitignored; nothing when offline, and the build goes on), and `build-whats-new.py` writes the
  page and the pill's `/usr/share/amethystora/whats-new.json`. It keeps a title only in plain words (no command, path,
  Markdown or upstream name; `test-whats-new.sh`), and the page is never translated (`20-tests.sh`).

### Translations

The Manual, Security, Updates, Logs, Notes, Control, Backups and Help follow the session's language, through
`apps/common/src/i18n.rs`. The English text in the code is the key: `t!("Update now")`,
`t!("Last updated {when}", when = ...)`, and plurals and variants in ICU MessageFormat
(`{count, plural, one {# file} other {# files}}`).

- The first argument of `t!`, `t_parts!` and `t_markup!` is always a string literal, a whole sentence or
  label: never fragments joined into a sentence. Text an app reads from data goes through
  `i18n::dynamic()` and is listed in `check-translations.js`'s `DATA`, which already lists the Manual's
  section names and every app's launcher.
- Each app's catalogs are `locale/<language>.json` in `system_files/shared/usr/lib/amethystora-<app>/resources/app`,
  where the Electron windows had them and Weblate writes them, keyed by the English. `locale/en.json`
  lists every message and is what Weblate translates from: after changing a message, run
  `node build_files/shared/check-translations.js --write --po po system_files/shared`, which reads the
  messages from `apps/` and rewrites it and `po/amethystora.pot`. `20-tests.sh` runs the same check
  without `--write`.
- Dates, numbers, sizes and lists go through `i18n` (`i18n::date()`, `i18n::days()`, `i18n::list()`,
  `i18n::number()`, `i18n::unit()`), never `format!`: ICU4X writes them as Chromium's Intl did, which
  `apps/common/tests/intl.rs` holds them to. Data that must read left to right in any language (paths,
  commands, ids) is set `gtk::TextDirection::Ltr`. GTK lays the rest out right to left for Arabic, and a
  stylesheet rule that names a side has its `:dir(rtl)` counterpart.
- The security report, the app permissions and Control's list translate with gettext (`gettext`,
  `eval_gettext` and `eval_ngettext`, values as shell variables), and the Security and Control apps show
  their words as they come. Never pass the report's or the helpers' text through `t!`.
- **The `ame` recipes** (`usr/share/amethystora/just/*.just`, each in `check-translations.js`'s `SHELL`
  list) say what they say through gettext too: `/usr/bin/ame` exports `TEXTDOMAIN=amethystora`, and a
  shebang recipe that prints or asks `source gettext.sh` after `set -euo pipefail`. A sentence is one
  message (a wrapped `echo` is merged), the values the user types (`on`, `off`, ids, `ERASE`), command
  names, `usage:` lines and anything a program parses stay as they are, and a `case` on what `gum choose`
  printed compares the translated labels (computed once into variables) as well as the ids.
- **Notifications are translated per account**, in `amethystora-notify-users`, not by the callers: the
  callers run as root, which has no session language. A caller marks a message with `N_ '...'` (and
  `N_n SUMMARY PLURAL '...' '...'` for a plural), exports the `${NAME}` values and passes the
  unexpanded text with `--msgid` (`--msgid-summary`, `--msgid-body`, `--plural` and `--count`, `--footer`
  for one more message after text that is not one); the helper takes each account's language from
  AccountsService (`Language=`, else `/etc/locale.conf`), runs `eval_gettext` under `LC_ALL=C.UTF-8
  LANGUAGE=...`, and only then cleans the text with `amethystora-security-text`. The label of every
  `--action` is translated the same way. What the security watcher found, the driver check's finding and
  the pools of disks' news are text of their own and stay English; only the sentences around them are
  messages. `check-translations.js` reads `N_`, `N_n` and `_('...')` (a GNOME Shell extension's own, the
  widgets', in domain `amethystora` through `"gettext-domain"` in its `metadata.json`).
- `translate-desktop.py` writes each launcher's translated `Name`, `GenericName`, `Comment` and
  `Keywords` from the app's catalogs at build time: edit the English in the `.desktop` file only.

### The Security app

Amethystora Security (`amethystora-security`, **Security** in the app grid) is the security report and
the virus scanner, in place of the report's terminal launcher and ClamUI. Its checks live in one
script, `system_files/shared/usr/libexec/amethystora-security-status`: `ame security status` prints
it and the app asks it for `--json`, so a check added or changed there reaches both. Never write a
check into the recipe or the app. The app runs only the commands that script names, in a terminal
(`amethystora-in-terminal`). What a scan finds is only reported unless `ON_DETECTION` in
`/etc/amethystora/security.conf` (default `report`) says quarantine or delete, and then only through
`/usr/libexec/amethystora-quarantine`, which scans each file again first; never add `--move` or
`--remove` to a scan. `/usr/libexec/amethystora-security-config` is the one reader of that file.
Starting the whole-machine scan without a password is all `50-amethystora-virus-scan.rules` allows.
clamd runs only while something scans: `clamd@scan.socket` starts it, in the foreground so that it keeps
the socket it is handed, and `amethystora-clamd-idle.timer` stops it when no scan and no real-time
scanning needs it. The socket's folder is Fedora's (0710 `clamscan:virusgroup`), so an account scans with
`clamscan`, as before.
Network protection, the app's **Network** page, is Suricata inline on the machine's own traffic
(`amethystora-ips.service`, `/usr/libexec/amethystora-ips`), off until `NETWORK` in that file says
`watch` or `block`; only `block` cuts anything off. It fails open at every layer: the queue's `bypass`
in `amethystora-ips.nft`, and the fail-open, exception and stream settings
`build_files/shared/ips-config.py` writes into the configuration it derives from the package's. Keep it
that way: a failure must cost inspection, never the network.

**The locked session** (`plans/done/PLAN-locked-session.md`): the system disk cannot close while the
session is only locked, so two things come close. `ame security usb locked`, USB protection's third state
beside `off` and `on`, is GNOME's own lock-screen USB protection: gnome-settings-daemon's plugin, in every
session on the screen and at the login screen, blocks new devices while locked and lets plain keyboards and
mice through. The image adds configuration only: `usbguard-dbus.service`, masked in every other state
(D-Bus would start it, and the plugin's allow-all rule would undo `on`), the three calls the plugin makes
granted to the active session (`50-amethystora-usb-lockscreen.rules`), and its two dconf keys locked. The
report and the recipe call it `locked` whenever `usbguard.service` is enabled and the D-Bus service is not
masked. The private folder (`ame security private-folder`) is `~/Private`, a gocryptfs mount of
`~/.local/share/amethystora/private`, and `/usr/libexec/amethystora-private-folder` is the one place it is
handled, always as the account. It is never open without `amethystora-private-folder-watch.service`, which
open starts before mounting and which closes the folder on `ActiveChanged(true)`, on `PrepareForSleep(true)`
under a delay inhibitor, and however it ends (`ExecStopPost`): it fails closed. The password goes from
gocryptfs's prompt or zenity straight into gocryptfs, never into a variable, argument or file. Closing
stops gocryptfs's scope without waiting for programs, takes away the folder's thumbnails and recent
files and closes, over the session bus so that it needs no shell, each Files tab in the folder or on a search (Files
never reloads a folder that was unmounted, even as a `GMount`, and a tab's number is its place in the row, which the
order Files publishes follows only until a tab is moved); closed, `~/Private` is 0500 (fusermount3 mounts only where its owner can write, so it is 0700 for the
moment of mounting). `test-private-folder.sh` checks it against stand-ins, `tests/vm/suites/86-locked-session.sh`
on a machine. The folder is also out of the file index (`Private` in localsearch's `ignored-directories`, in zz1), which Fedora 45 otherwise fills from the whole home
with the mount's names and text.

The security watcher (`amethystora-security-watch`, on a 15-minute timer, or on each audit event when
`REALTIME_WATCH=on`) reads the audit log per key, the journal, `/etc` against `/usr/etc`, setuid files
outside `/usr` and listening ports, writes what only root can see to
`/var/lib/amethystora/security/status.json` for the report, and runs the report to notify when a watched
check turns bad. It also looks at what each account's session finds first: a command in `~/.local/bin`,
`~/bin` or Homebrew's prefix named like a system one, and a launcher in `~/.local/share/applications` that
no container export made. An autostart entry written as exactly `[Desktop Entry]` and `Hidden=true` (Control's
Starting with you, `/usr/libexec/amethystora-startup`) starts nothing and is recorded quietly (`hidden_autostart`);
anything more there is said. The app grid hides and renames apps with keys, never a launcher. The audit rules ship one file per key in `/usr/share/amethystora/audit`, which
`amethystora-audit-home-rules` writes into `/etc/audit/rules.d/60-amethystora.rules` at every boot, beside
the per-home rules (`persistence`, `user-programs`). Where the kernel takes it, the **file-watch sensor**
records those keys instead, without audit's per-system-call cost (`apps/sensor`, `amethystora-sensor`,
`plans/done/PLAN-audit-cost.md`): the generator probes it once a boot and, for every key it covers,
writes a `/run/amethystora/sensor/{plan,watch.list}` line from the same shipped rules rather than an audit
rule, so the kernel loads only `-a never,task`; the watcher then reads that key from the sensor's own log
(`sensor-KEY` checkpoint) and auditd's for the rest. A kernel the sensor cannot load on, or `ame security
audit-locked`, falls back to the audit rules unchanged. Nobody logs in before it is watching (`Type=notify`,
before `systemd-user-sessions.service`, 30 s at most), and its path walk and `put_program` are BPF global
functions, which the verifier checks once: inline, they made its load take 14 s. An `fexit` program's
arguments must be the running kernel's (its BTF), the result last. Since 6.19, `vfs_mkdir`, `vfs_rmdir`, `vfs_mknod` and
`vfs_symlink` take a `struct delegated_inode *` before their result. A program one argument short reads that pointer as
the result, which recorded every such write as failed, and the watcher keeps failures quiet. `tests/vm/suites/32-sensor-parity.sh`
makes and removes a folder and a symlink to hold this. A new audit key has to be
added to the watcher, to `amethystora-security-realtime`, to `amethystora-security-config` and to the sensor
(`src/bpf/sensor.h`'s keys and `src/keys.rs`); `20-tests.sh` fails otherwise, and checks
`amethystora-sensor --keys` against the watcher's. The app's **Containers** page is what
`amethystora-pkg containers list --json` says, run as the user.

**Every security feature is a switch, and off costs nothing** (the maintainer's rule of 2026-10-08). A
detection feature is a key in `security.conf`, defined in `amethystora-security-config` with its values,
words, group and how it applies (`--list --json`, the app's **Settings** page, `ame security settings`);
every unit that runs one reads its key in `ExecCondition=` (a drop-in for the packages' own: `clamd@`,
`clamav-freshclam`, `auditd`, `audit-rules`), and a key that is off writes no audit rule, so a line edited
by hand applies at the next boot. What the image enforces from the start (kernel settings, blocked
modules, FireWire, kernel arguments, firewall zone, fail2ban, lockout, the Flatpak override, virtual input, SSH
settings, signed updates) is switched by `/usr/libexec/amethystora-hardening`, one `ame security` command each, which keeps
what is off in `/etc/amethystora/hardening-off` and names the files it writes, which the watcher's drift
check accepts exactly. A feature switched off is `info` in the report with the command that turns it on,
never a warning; turned off any other way it still warns. A change made through a switch is noted
(`amethystora-security-config note`), and the watcher records it without notifying. `ame security
profile off|default|light|balanced|thorough` sets every detection key at once; no profile turns on, or
(but `off` and `default`) touches, a key whose `SENDS` is more than downloads, and each of those carries an
`EXPLAIN` that is shown and asked before it is turned on. `ame security allow report` accepts a check that
cannot be fixed on the machine, and `ame security allow KEY ENTRY` quietens a path or program a feature
keeps finding (`/usr/libexec/amethystora-security-allow`, which holds each feature's entry shape). Every
finding has an id, and a fact `allow=` when it can be allowed: **Allow…** on the Events page sends the id
only, and the program passes the entry the event holds. `test-security-switches.sh` checks all of it. A new
feature gets its switch with it, off by default, and a category: the app's Settings page shows the switches by
the `group` each helper gives them (`GROUP` in both), a category a page, whose ids and words are `categories()`
in `apps/security/src/page.rs`. Its **Under duress** page only starts `ame security duress` steps in a terminal
and reads nothing, so that it looks the same whether an account has a duress finger or not.

**What the desktop shows of it** (`plans/beyond/spec-r4-privacy.md`). Amethystora UI's shield reads
`/var/lib/amethystora/security/report.json`, which the watcher writes `root:wheel 0640` for it, and the settings through
`amethystora-security-config`. It switches nothing: each line opens `amethystora-security settings/<category>`
(`backend::CATEGORIES`, the Settings page's eleven, never the Under duress page) or the overview, and its reader drops every
`duress-` check (`HIDDEN` in `privacy/readings.js`, which `20-tests.sh` holds). **Walk-away lock** (`WALK_AWAY`, group
`logins`, off; `ame security walk-away`) is a security switch the shell carries out (`privacy/away.js`): the screen locks
once every paired phone that was connected has been gone a minute and the computer left alone a minute. It never unlocks,
and the phone link stopping is not a phone leaving. No profile but `off` and `default` touches it (`away()` in the config
helper), it is never in the setup file, and the agent may turn it on and never off.

**What looks for new threats** is the watcher's too, each part a key in group `detection` (or `network`,
`scans`), off as the image ships: `PROCESSES` (/proc read in a few bounded passes, never a fork per
process), `SELINUX_DENIALS`, `SNAPSHOT_WATCH` (`btrfs send --no-data` between hourly snapshots), `EXEC_WATCH`,
`AGENT_CLIS`, `CREDENTIALS` and `DECOY` (audit keys of their own, whose rules, or the sensor's watch-list
lines, the home generator writes for paths that exist, and the watcher reads from raw records with
`audit_records`), `BPF_PROGRAMS`, `CHAINS`,
`DNS_NAMES` (`amethystora-dns-names.service`, resolved's monitor), `BEHAVIOUR` (Falco, from falcosecurity's
pinned repository, run only by `amethystora-behaviour.service`; 0.45.0's probe does not load into Fedora 44's
kernel, so `20-tests.sh` fails below 0.45.1), the `YARA_*` sources (`amethystora-yara.service` as clamupdate, which splits and test-loads every
set, since one rule ClamAV cannot load can stop its whole database) and `NETWORK_FEEDS`/`NETWORK_HUNTING`
(`amethystora-ips due`). What only an account can see is read as that account by
`/usr/libexec/amethystora-security-reader` (`APP_PERMISSIONS`, `DEV_HOOKS`, `AGENT_CONFIG`), run with
`setpriv` and `timeout` for the accounts with a session in the last day, root keeping what it saw, and said to
that account alone (`amethystora-notify-users --user`). `OSPREY`, `TRIAGE` and `REPUTATION` send something
away and are in no profile. What turns each agent's checks off is `/usr/share/amethystora/agent-cli-flags`,
which must name every agent `amethystora-agent` offers. A finding carries `at=` (the snapshot Backups opens,
`--at=POINT`) and `tell=` where they apply; one the allowlist quietened carries `allowed` and builds no chain.
`test-security-watch.sh`, `test-security-reader.sh` and `test-yara.sh` plant each kind of finding. A library
loaded by `LD_PRELOAD` from outside `/usr`, and a session whose user manager gives every program a variable of
`/usr/share/amethystora/code-variables` (`systemctl --user show-environment`, run as the account with `setpriv` on its
manager's own socket: `-M USER@.host` from a system service starts a unit for every question), are `PROCESSES`'
too; the same list is the app permissions' `code` reach. `INJECTION`, an audit key of its own, is every attach to,
or read or write of, another program's memory by a login: the sensor's `fexit/security_ptrace_access_check`, which
returns at once unless the mode attaches (every `/proc/PID/stat` read asks it too), records the result, the call and
the program reached, and an `OBJ_PID` record with both mount namespaces (`omnt`, `mnt`); `injection.rules` is the
audit fallback, which sees no `/proc/PID/mem`. A program's own business (the same program, a neighbour outside the
shared folders, one sandbox other than the host's) and `/usr/share/amethystora/debuggers` are recorded only.

**App integrity** (`APP_INTEGRITY`, `/usr/libexec/amethystora-integrity`, Python with the OSTree bindings and
SQLite; `plans/done/PLAN-app-integrity.md`) checks the system's ostree repository, each deployment's
`.ostree.cfs`, the kernels and initramfs images in `/boot`, and every Flatpak installation against what their
signed updates installed, daily and on `/var/lib/flatpak/.changed` (`amethystora-integrity.timer` and
`.path`). Updates verify what they write, so root's files are trusted the first time and kept by their stat
(`integrity/system.db`), and only a changed ctime is read; an account's own installation is checked by content,
read as that account with its rows, root's, handed to it on a descriptor and its changes printed back. A
deploy's tree is compared with its commit, and its signature by fingerprint (Flathub's or a system remote's
keyring's), since ostree trusts every keyring in the repository's folder; what Flatpak writes itself (its
launchers, whose `Exec=` has to be its own `flatpak run` of the app, the `export/bin` wrapper, the deploy data,
extra data) is kept as written, and every entry of an installation's `exports` has to lead into an active deploy.
Its findings (`integrity/findings.jsonl`, all of them each run) reach the watcher as a reader's would, and
`status.json`'s `integrity` the report's `app-integrity` check; none can be allowed. A finding about the
system's own files means reinstalling: nothing repairs those, and `ame security app-integrity repair` only
fetches an app again. `test-app-integrity.sh` plants each kind of change against stand-ins.

The same helper holds the stricter settings, each off until turned on and kept in
`/etc/amethystora/hardening-on`: `kernel strict` (io_uring, ptrace, address randomisation, IPsec), extra
kernel arguments (`EXTRA_ARGS`; `proc_mem.force_override=ptrace` was one and is now the image's own in
`kargs.d`, which `system-setup.hooks.d/28-proc-mem.sh` adds where rpm-ostree updates), `time` (chrony with NTS only), `firewall-away` (the `amethystora-away`
zone by default, the networks named home keeping theirs), `network-daemons`, `block-bluetooth`,
`block-webcam`, `block-automount` and `block-user-extensions` (locked dconf keys in `local.d`),
`block-xwayland` (`--no-x11` in GNOME Shell's user unit, whose `ExecStart` `20-tests.sh` compares),
`flathub-verified`, `flatpak-password`, `noexec-temp` (marked `/etc/fstab` lines), `no-coredumps`,
`package-cooldown` (npm, pnpm and uv through `environment.d` and `profile.d`), `brew-attestations`,
`boot-password` (`grub2-setpassword`, which bootupd's static `01_users.cfg` reads), `locked-poweroff`
(`amethystora-locked-poweroff.timer`, enabled only while it is on, reads logind's `LockedHint`) and, on dx,
`vscodium-extensions` (`/etc/vscodium/policy.json`, which VSCodium reads under that name). Each says what
it costs before it is turned on; the report's `stricter` check names the ones that are on, and the watcher
says when one is turned off any other way. `ame security lockdown strict` is lockdown's own third level.

**What the machine sends by itself** (`plans/done/PLAN-what-leaves.md`) is said by `/usr/libexec/amethystora-sends`
(`ame security sends`; its records are the report's `sends` group and the Security app's last card, information
whatever their state, so every reader of "state off" filters the group out as it does `optional`). Three of them
are `amethystora-hardening` switches, on until turned off, with a `SENDS` table of what each governs: `countme`
(masks rpm-ostree's weekly count of Fedora's machines, a timer Fedora's preset enables; sealed images never send it),
`connectivity-check` (a NetworkManager drop-in with `enabled=false`; the row reads what NetworkManager does, since
GNOME's own switch is kept apart from `/etc`) and `firmware-metadata` (masks `fwupd-refresh`); location is the
account's `ame security location`. Nothing is turned off for the owner, no profile touches them, and a new thing that
contacts the outside by itself goes in the helper with its switch. `tests/vm/suites/44-what-leaves.sh` watches
the network for a host the list does not name.

**Phone pairing is open only at home** (the maintainer's choice of 2026-10-08). The firewall has three
zones: `amethystora`, the default, which every network gets; `amethystora-home`, the same with firewalld's
`kdeconnect` service (the phone link's ports, see The phone link), for the saved connections named home (`ame security home-networks`, kept in
`/etc/amethystora/firewall-home`, applied as each connection's `connection.zone`); and `amethystora-away`,
the default instead while `firewall-away` is on. With the Amethystora zone off, the home networks go back to
the default zone and the list stays. fail2ban bans in all three through the image's own
`action.d/amethystora-zones.conf`, since fail2ban's `firewallcmd-rich-rules` bans in the default zone alone.
The phone link listens only on interfaces in `amethystora-home`.

Anaconda adds `ssh` to the default zone unless told not to: `firewall --use-system-defaults` in `iso/live/installer.ks`, which `build.sh` holds. `iso/iso.toml`'s installer has Anaconda's Network module off, so has no such step and would not take the command. `system-setup.hooks.d/31-installer-ssh.sh` takes it from machines installed before that, once, unless sshd is enabled.

**What an account plants is only ever read.** Text somebody else wrote (a file name, a program's name
or command line, a launcher's `Exec=`, a host name, a rule's message) goes through
`/usr/libexec/amethystora-security-text` before it reaches a notification, a history line or a terminal:
`amethystora-notify-users` puts every notification through it, the watcher every finding, the scans every
file name they log, and the agent's prompts quote such text between lines of dashes as data. The watcher
tells only administrators (`--admins`), since its findings name other accounts' files. In a home the
watcher reads only plain files, never through a link, under `timeout`, and its unit has a
`TimeoutStartSec`. A finding carries its time, level, the part that found it (`key`), its account and
ATT&CK tactic: the history is JSON lines in `/var/log/amethystora-security-events.jsonl`, and
`events.json` beside `status.json` (root:wheel 0640) is the Security app's **Events** page.
`test-security-watch.sh` plants such text and checks every place it reaches.

What each Flatpak can reach beyond its sandbox is `/usr/libexec/amethystora-app-permissions --json`,
run as the user, which the report's **App permissions** check, the app's **Apps** page and `ame
security apps` all read. It compares `flatpak info --show-permissions` (every override applied) with the
app's manifest and its own overrides, and gives each reach an id from its fixed list; the report flags
the ids that leave the sandbox, whoever granted them. The app takes back only the account's own
overrides for an app the list says has some (`flatpak override --user --reset`), and takes one app's
network away or gives it back the same way (`--unshare=network`, `--share=network`), only for an app the
list says has it or had it taken. The global override
the image ships, `/etc/flatpak/overrides/global`, only applies through the link
`amethystora-flatpak-overrides.conf` makes in `/var/lib/flatpak/overrides`: Flatpak reads no other place.
It takes X11, the input devices and the Flatpak service from every app, and leaves `--device=all` to the
apps that ask for it.

Encrypted DNS (`ame security dns`, `/usr/libexec/amethystora-dns`) and a different address on each
network (`ame security mac [off|away|on]`, `/usr/libexec/amethystora-mac`) are switched by the files their helpers write,
not by `security.conf`: a resolved drop-in with the resolver, `DNSOverTLS=yes` and `Domains=~.`, and
NetworkManager drop-ins copied from `/usr/share/amethystora/network`. The DNS one sets each network's own
servers to plain text, for its own names and the connectivity check, which would otherwise find every
network offline. Three things in the helper were settled from the programs' source, and `20-tests.sh`
holds them: systemd-resolved is reloaded, never restarted, since a reload keeps what NetworkManager and
Tailscale set on each network and a restart forgets it; networks connected already are changed with
`nmcli device modify`, since `nmcli device reapply` on an unchanged connection rebuilds nothing; and on a
network that wants a login first, the user unit `amethystora-dns-portal.service` (in the session, not a
NetworkManager dispatcher script) offers a ten-minute pause, which the person at the machine starts:
never pause it automatically, or any network could turn the encryption off.
The sidebar draws the boot splash's wordmark live, from `apps/common/assets/gem.svg` (written by
`branding/generate.mjs`) and Quicksand, with the generator's own sizes and glow.

### Logins

The image's authselect profile carries `with-pam-u2f` (`04-packages.sh`), `with-faillock` and
`with-fingerprint` (`08-hardening.sh`, the installer's own feature last), and no profile change may drop
them. realmd's stock login commands would (`authselect select sssd with-mkhomedir --force`), so
`/etc/realmd.conf` runs `/usr/libexec/amethystora-domain-logins` for all four: a join selects `sssd` or
`winbind` with the image's features and `with-mkhomedir`, and `realm leave` the image's own profile again,
with the image's `pam_winbind.conf`, which a winbind join edits and realmd does not put back.
The watcher takes that setup, and nothing else, as the image's (`amethystora-domain-logins check`): exactly
the features a join turns on, and every file in `/etc/authselect` as `authselect test` prints it, byte for
byte. Never rely on `authselect check`, which calls a file with a line added by hand valid. A
domain's accounts are not in `/etc/passwd` and are numbered far above 65534, so the per-account checks
find accounts by their folders in `/var/home` as well, and never filter on `uid < 65534`.

- **Parental Controls' allowed hours** are `pam_malcontent` in `login` and `sshd` only, by the
  maintainer's choice of 2026-10-05: GNOME Shell's own lock covers the desktop and keeps unsaved work,
  where the module would close the session. Keep it out of GDM's services, and keep its line before
  `account include`, where the `sssd` and `winbind` profiles' `sufficient pam_localuser.so` cannot end
  the stack ahead of it.
- **`ame security ssh-codes`** (off by default) makes a password login over SSH ask for a code from
  google-authenticator too. Its state is the two files `/usr/libexec/amethystora-ssh-codes` writes, an
  sshd drop-in and one line in `/etc/pam.d/sshd`, deliberately not a key in `security.conf`, which sshd
  cannot read. The watcher accepts exactly those two and nothing in between.
- **Both rest on `amethystora-pam.cil`**, an SELinux module `08-hardening.sh` installs. Fedora's policy lets
  neither the console's `login` nor `sshd-session` ask accountsservice, so without it `pam_malcontent` fails
  open and a child logs in at any hour; and it labels google-authenticator's new secret file only under a
  name the module no longer uses, so every login with a code fails. Neither shows in a build:
  `tests/vm/suites/83-logins.sh` makes each login as a person would.
- **`ame security duress`** (off by default) sets up a duress finger: one more finger that unlocks at the
  login and lock screens (`duress=allow` in `fingerprint-auth`), is refused at sudo, polkit and the text
  consoles (`duress=deny` in `system-auth`), and runs the owner's action as root either way, once per boot.
  pam_fprintd tells the finger apart only in the image's own fprintd build (`packaging/fprintd`, built in
  the Containerfile's `fprintd` stage): upstream's `VerifyFingerMatched` signal and its signals sent only to the
  claiming client, both unreleased after 1.94.5, plus Amethystora's patch to pam_fprintd. The feature
  `with-duress-fingerprint` is the per-machine switch: `08-hardening.sh` turns the link to `system-auth` and
  `fingerprint-auth` in the vendor `local`, `sssd` and `winbind` profiles into copies with one `{if}` each,
  so that with it off authselect writes the same bytes as before, and `amethystora-domain-logins` keeps it
  through a join or a leave. Under SELinux, GDM's and polkit's PAM may neither start a unit nor read
  fprintd's prints, so the module only reads `/etc/amethystora/duress.d/fingers` and sends `Run(uid)`,
  without waiting, to `amethystora-duress-service`, which runs from boot while the feature is on and has
  `/usr/libexec/amethystora-duress run UID SOURCE` check the sums of that source's action and of the print
  before anything runs. **A duress password** is the same feature for a typed password: the per-machine
  switch `with-duress-password` adds one `pam_exec.so ... /usr/libexec/amethystora-duress-pam` line after
  `pam_unix` (before faillock's authfail) in `system-auth` and `password-auth`, in all three vendor
  profiles, marked `{include if}` so that with it off authselect writes the same bytes as before (a false
  `{if}` leaves an empty line). That helper
  reads only `/etc/amethystora/duress.d/passwords` (`UID` and a yescrypt hash, never the password), decides
  allow or deny from `PAM_SERVICE` (only `gdm-password`, the login and the lock screen, unlocks; everywhere
  else is refused like a wrong password), and sends `RunPassword(uid)` (the finger sends `Run(uid)`). **The
  finger and the password each have their own function**: each runs its own script, `action-fingerprint` or
  `action-password` (an unrestricted root-level script, `env -i` but no time limit, with `DURESS_SOURCE` set),
  fires once per boot on its own, and is reported under its own check (`duress-finger`, `duress-password`);
  they share only the folder, the log, the service (`Run`/`RunPassword`) and the switch surface, and either
  feature turns the service on. A duress password needs no fingerprint reader. **Nothing may reveal whether an
  account has a duress finger or password**: no key in `security.conf`, nothing in the journal
  (`LogLevelMax=emerg`), one file for every account, the report and `status.json` silent until something keeps
  an action from running, and the agent skills told never to touch or mention it.

### The Updates app

System Updates (`amethystora-update`) replaces upstream's System Update launcher, which opened a
terminal on `ujust update`; `ame update` stays for the terminal. **Update now** starts
`uupd-manual.service`, the same updater the automatic updates run, which the person at the machine
may start without a password. The `uupd.rules` the uupd package ships (the ublue-os/packages build
installs it in `/etc/polkit-1/rules.d`, not `/usr/share`) allows every verb on both of uupd's units to
anyone, `stop` and `set-property` included, so `40-amethystora-uupd.rules`, read before it, allows only
`start`, and only locally; anything else takes an administrator's password. The window never runs `bootc`, `flatpak` or `brew`
itself, with one exception: on a machine started from an older deployment than its default one,
**Keep this version** runs `pkexec bootc rollback`, after an administrator's password
(`amethystora-update-alert.service` says so at login and opens the window). It follows uupd's JSON log
in the journal and reads the deployments from `rpm-ostree status`. If uupd changes the `Updating`,
`module_fail` or `Updates Completed Successfully` messages it logs, update `apps/update/src/backend.rs` to match.
**What is waiting** (`/usr/libexec/amethystora-update-check`, `amethystora-update-check.timer`, daily and after each start,
`ame update check [on|off]`) looks as root without installing anything: the image against the registry's by digest (skopeo, as
uupd's rpm-ostree driver does, since `rpm-ostree upgrade --check` does not say it reliably) and, on a machine with layered
packages, the layered entries (type 1) of that check's cached `rpm-diff`, every day whether the image changed or not. It writes
`/var/lib/amethystora/updates/available.json` (0644), which the window's **Waiting to be installed** shows only for the deployment
it names and with nothing staged, and its notice unit says it once while `uupd.timer` is off. uupd's two units are `After=` it so
that the two never hold rpm-ostree or bootc at once, and polkit lets the person at the machine start it (Check now). Its `driver`
(bootc, or rpm-ostree when bootc calls the booted or staged deployment `incompatible`, uupd's own test) is what `ame update` uses.
It is a row of `amethystora-sends`; a sealed machine does not look. `test-update-check.sh` and suite 73 check it.
**Restart now** finishes an update with a soft reboot (systemd's, the system's programs only) when it keeps the
kernel, its initramfs, its arguments and the SELinux policy as they are: `/usr/libexec/amethystora-soft-reboot`
says when (bootc's `softRebootCapable`, then no loaded driver in another version and nothing waiting on the
firmware; never on a sealed image), mounts the staged deployment at `/run/nextroot` (`ostree admin
prepare-soft-reboot`) and asks logind for the restart as the user, which logind then makes a soft reboot, with its
own polkit questions still asked. Before that it adds `x-initrd.attach` to the crypttab lines of the LUKS partitions
the system runs from, which Anaconda leaves out: without it their cryptsetup unit stops at shutdown and takes the
update's mounts under `/run/nextroot` with it, and the machine comes up in emergency mode (ostreedev/ostree#3503,
found in the lab). `50-amethystora-soft-reboot.rules` lets anyone ask its status, and the person at
the machine prepare one, without a password. GNOME's restart dialog only restarts the computer, so the window asks
first itself, and leaves the restart to that dialog when an app inhibits logging out. `ame update restart` is the
same in a terminal. An update is only ever quick because `19-initramfs.sh` keeps out of the initramfs what changes
with every build (the build's version in os-release, the out-of-tree drivers modules-load.d names): keep it so
(`20-tests.sh` checks, and `tests/vm/suites/89-soft-reboot.sh` stages derived images and restarts through the
window). After an
update started there, it starts the account's `amethystora-pkg-upgrade.service` and shows it as the
**Containers** step, which reuses the step the window keeps for uupd's own distrobox module; a
container's failure shows there only, never as a failed system update. Its **Firmware** section asks
fwupd (`fwupdmgr get-updates --json`, no password) and installs in a terminal (`fwupdmgr update`), where
fwupd asks before a restart; `fwupd-refresh.timer`, which `17-cleanup.sh` enables because Fedora leaves
it to GNOME Software, fetches what LVFS has. `amethystora-update-alert` says at login, once for each
release, that firmware waits, and the security report's `firmware` check names it. The top bar draws the same wordmark as Security.

**After a restart** (`plans/ui-ideas/spec-restart.md`). Before an update's restart (`amethystora-soft-reboot restart`,
the window's full restart, the night restart) the open apps are kept by Amethystora UI's workspaces module through
`org.amethystora.UI.Resume.Save()` on the session bus, as app IDs and places only (`~/.local/state/amethystora/resume.json`,
read and deleted by the next GNOME Shell, offered in the pill only when the system's version changed); a lock while an
update is staged keeps them too (the extension's `beforeDisable` hook, while the tiling still runs), and the unlock
deletes them. **Restart while you're away** (`/usr/libexec/amethystora-night-restart`, `ame update night-restart` and
`ame update tonight`, off) is a transient timer of the account that restarts only with the screen locked, nothing
inhibiting the logout, on mains and within the hour, never waking the machine; it is never in the setup file, since a
shared file must not schedule another machine's restarts. **Saving data**: `uupd.service` (a drop-in), and the user units
`amethystora-pkg-upgrade.service` and `amethystora-backup.service`, ask `/usr/libexec/amethystora-metered wait WHAT` in
`ExecCondition=`: a run its timer started waits on a metered connection (NetworkManager's 1 or 3), at most a week by the
stamp of its first wait, and a backup to a local disk never; `uupd-manual.service` (Update now, the pill's Update anyway)
has no condition. Keep both: the waiting must never stop an update a person asked for, nor last longer than a week. **Get me through** (`amethystora-metered through on|off`) makes the account's timed backup and containers' upgrade wait on battery too: its mark holds
the shell's process number and counts only while that is still `gnome-shell`, it shares the week's stamp, and a person's own run and a backup to a local
disk never wait. The system update has no mark: root must not take a file an account writes as a reason to delay updates, and uupd's own 20% battery check does it.

**An update that does not start goes back** (R7-52, `plans/ui-ideas/r7/spec-sturdy.md` 3; `ame update boot-guard on|off`,
`/usr/libexec/amethystora-boot-guard`, `amethystora-boot-guard.service` and `-good.service`) is **built and off** (`/var/lib/amethystora/boot-guard/on`
is absent), and stays off until the lab and a real machine have proved it: that is the maintainer's. It is never in the setup file, and sealed images
are left alone. **Its bar: a machine that started fine is never sent back, and no sequence of power cuts leaves one unable to start by itself**
(`test-boot-guard.sh` cuts the power after every step it takes, against a model of ostree's boot version and GRUB's script). GRUB's own counter
cannot be used: bootupd's static configuration drops `08_fallback_counting` and `10_reset_boot_success`, and greenboot is not in the image, so it
counts in `grubenv` from `custom.cfg`, the one hook, which `amethystora-raid` writes too (both under `/run/amethystora/boot-cfg.lock`). `set
default=1` would be UEFI Firmware Settings. A good start is the display manager up and a session on seat0, not a login. It arms at an update's
shutdown only after a good start, when the new default carries the guard, and when the two entries differ by more than ostree's numbering. At the
new system's good start `good` removes the entry before anything else; the rollback goes by where the default is, never by what a command said.
While it holds an update back (`update-ok`, an `ExecCondition` of `uupd.service` alone; Update now never asks) Flatpak and Homebrew updates wait
too, which the notice and the manual say. The agent's never list holds its switch, helper, state and `grubenv`. Between it and on: a lab run that
breaks an image on purpose, a real boot (`save_env` under Secure Boot, GRUB's `/loader.N` and kernel tests, `fsfreeze` on `/boot`), and the maintainer
weighing that anyone at the power button can hold a machine on the old system with automatic updates paused.

### The Logs app

Amethystora Logs (`amethystora-logs`, **Logs** in the app grid) replaces GNOME Logs, which
`amethystora.preinstall` leaves out and `system-setup.hooks.d/21-retire-gnome-logs.sh` uninstalls,
once, from a machine that already had it. It is read-only: it runs
`journalctl` as the user, and nothing else but a terminal, the Manual and the agent's diagnosis on what
the program picked, so administrators (`wheel`) see the whole machine and anyone else their own session.
`apps/logs/src/backend.rs` builds every `journalctl` argument from the window's choices: an app has to be
one it found with `journalctl --field`, a unit or program only ever reaches it as the value of `--unit`,
`--user-unit` or `--identifier`, and the search only as the value of `--grep`. Keep it that way, and
never pass a string typed in the window as an argument of its own. Its manual section is
`troubleshooting.md#the-logs`. The sidebar draws the same wordmark as Security, and blinks the gem's stem
while it reads the journal.

### The Notes app

Amethystora Notes (`amethystora-notes`, **Notes** in the app grid) is the notes and to-do app, in
place of Joplin and Planify: notebooks, Markdown notes with tags, attachments, history and a trash,
and tasks in an inbox or projects with sections, dates, repeats, reminders, priorities, labels and
subtasks. It imports Joplin's JEX export and Planify's JSON backup. Nothing is synced and it opens no
connection: keep it that way, and keep Joplin and Planify off the Flatpak lists (`20-tests.sh`
checks). Everything is in `~/.local/share/amethystora-notes`. When encrypted (offered, and chosen by
default, on first run), the data and each attachment are sealed with AES-256-GCM under a random key,
which is itself sealed under a key derived from the passphrase with scrypt. An encrypted vault reads
only sealed files. Turning encryption on or off and a new key (**Change the key as well**, on by default
when the passphrase changes) write everything into `next/` first, then `vault.json`, and move into
place only what the vault's key then fits. The program holds the key only while unlocked, and opens an
attachment only to show or save it. A note's Markdown is rendered as the Manual's is
(`apps/common/src/markdown.rs`), with raw HTML shown as text. Keep both properties. The files are the
ones the Electron window wrote, in the same formats: `apps/notes/tests/compat.rs` opens what that
window wrote, and has its own code (`electron.js`, on the apps stage's Node) read what this one writes.
Its manual page is `notes.md`. Its lock screen lifts the gem's stem while the passphrase is checked.

A passkey (**Settings**, **Add a passkey**) opens the notes beside the passphrase, never in its place:
the same random key sealed once more, under the secret a FIDO2 security key computes with `hmac-secret`
for the credential and salt kept in `vault.json`. It asks the security key through `fido2-token`,
`fido2-cred` and `fido2-assert` (`fido2-tools`, in `04-packages.sh`), which take their parameters and
the PIN on stdin, and always with its PIN or a fingerprint (`pin=true` or `uv=true`): a touch alone
must never open the notes. The credential is made with `credProtect` level 3 (`fido2-cred -c 3`), and a
key without `credProtect` is refused: a key of the first FIDO2 generation may otherwise give the same
`hmac-secret` for a touch alone. Such a key will not say whether it holds a credential without the PIN,
so unlocking tries each passkey in turn. Nothing is stored on the security key, and a backup carries
no passkey.

### The Vault

Amethystora Vault (`amethystora-vault`, **Vault** in the app grid, `ame vault`; the design is `plans/PLAN-vault.md`, its brief
`plans/vault/IMPLEMENTERS.md`) keeps passwords, authenticator codes, backup codes, keys, certificates and protected files, away from the
programs of the account that owns them. **A secret is out of the account's reach, and a program of the account that asks needs the person's
approval**: those two halves are the whole design, and every rule below serves one of them. Its manual page is `vault.md`.

- **The crates** (all in `apps/`, one Cargo workspace): `vault-core` (no GTK, no socket: items, sealing, slots, the store, generators, the
  importers, `keys` and `pki/`, the protocol), `vault-service` (`/usr/libexec/amethystora-vault-service`; `amethystora-vault-cli`, which
  `vault.just` runs; `amethystora-vault-work`, the short-lived child that is the only one of them linking libcrypto; and
  `git-credential-amethystora` and `docker-credential-amethystora` in `/usr/bin`), `vault-client` (the wire format Round 7's features seal
  under: `seal`, `open`, `forget`; keep it exactly), `vault` (the window, app id `org.amethystora.Vault`),
  `vault-pkcs11` (a PKCS#11 module with no key in it, which lets `pdfsig` sign through the socket) and `vault-net` (the network service).
  The sentences polkit shows are `say` in `vault-service/src/outside.rs`.
- **The service** is a system service under its own user (`amethystora-vault`, `usr/lib/sysusers.d`), started by `amethystora-vault.socket` and
  `amethystora-vault-ssh.socket` (both enabled, every account may connect: `SO_PEERCRED` picks the account, never what a client says), and gone
  within a minute of the last vault closing. Its unit has `PrivateNetwork=yes`, `CapabilityBoundingSet=`, `MemoryDenyWriteExecute=yes`,
  `ProtectSystem=strict`, `ProtectHome=yes`, `SystemCallFilter=@system-service`, `LimitCORE=0` and `LogLevelMax=warning`, and its state is
  `/var/lib/amethystora/vault/<uid>` (0700, the service's) and `machine/`. **Why not a user unit:** an account can override its own units with a
  drop-in, start them under `LD_PRELOAD` and read their memory. `test-vault.sh` holds every line of the unit.
- **Approval is polkit**, `org.amethystora.vault.{open,use,export,admin}`, each owned by the service's user (which is what lets it ask for the process
  that connected and pass the sentence: `polkit.message` with `$(program)`, `$(item)`, `$(detail)`), `allow_any` and `allow_inactive` no,
  `auth_self` (`auth_admin` for `.admin`), **no `yes`, no `keep`, no rule that lets anyone through**. A forced ask records no yes, so the item
  asks again; the service never asks while the account's session is locked (the dialog would show over the lock screen) and cancels what is pending on
  lock, sleep and a client hanging up. The name in a prompt is what the program calls itself (`comm` and `cmdline[0]`, a Flatpak's app id from its
  cgroup): it proves nothing and is never used to skip an approval. Every `# N_` line of `vault.just` must equal a `say` sentence, which is how the
  prompts are translated (the service says them in English, polkit's gettext domain `amethystora` translates).
- **The rules that must hold.** The service never links libcrypto (8 MB resident, no parser in the process that holds every vault's key: keys, X.509 and
  PKCS#12 run in `amethystora-vault-work`, one job each), never has a network, never panics on a request (`panic = "abort"`), and holds secrets in
  `Zeroizing` types that wipe. Everything an outside party names (an id, a purpose, a path) is checked for its shape before it touches a path; writes
  are atomic; the same refusal for "no such item" as for "wrong key". A client's connection is dropped after 5 s idle, so every client reconnects for each
  call. Round 7's unattended `background` key is served to a lingering account, and nothing else is. No `rsa` crate anywhere (RUSTSEC-2023-0071;
  `test-vault.sh` fails on one in `CRATES`).
- **Several and hidden vaults.** The first vault is the only one with the account's CA, its certificates, sync and Round 7's `data-key`s. A hidden vault is in no
  list, log, error, report or `status.json`, opens by its password only (one scrypt salt for all of an account's vaults, so time says nothing), never fetches a
  logo, is never offered sync, is not in `amethystora-sends`, and `ame vault forget` says "including any you cannot see" whether there is one or not. It is
  **not the duress feature and is never tied to it**. Do not add a place where a hidden vault shows.
- **Certificates.** Each machine makes its **Amethystora OS Root CA** on first need (`OU=Machine` and eight hex digits of its key, never the host name or
  `/etc/machine-id`), RSA 4096, in `vault/machine/`, in no backup and never exported; it signs only what a table in the service says, through three CAs
  (an account's Code Signing CA, the machine's `Device Identity CA` and `Local Services CA`), **each with its own critical Name Constraints** (a constraint above a
  trust anchor is never read: tested in Firefox). It never joins `/etc/pki/ca-trust`. A PDF certificate and a code certificate are two certificates (poppler
  accepts neither `codeSigning` alone nor a CA limited to it). The key signs inside the service; `apps/vault-pkcs11` hands `pdfsig` a digest and gets a
  signature, and `ame vault pdf sign` always passes `-nssdir`. `ame vault root rotate` is the only revocation there is.
- **Using a secret without handing it over**: the SSH agent (`amethystora-vault-ssh.socket`, the same process; `ame vault ssh on` writes `environment.d`
  and a drop-in for `gcr-ssh-agent.socket`), tools' own helpers (`ame vault helpers`: they write a tool's settings only when asked and never over someone's own),
  `ame vault run` (a user, mount and PID namespace with a tmpfs over the file's folder; **inside it there is no `sudo` or `pkexec`, and other programs of the
  account can reach what it holds**, which the manual and `protect` say first) and protected files (kind `protected`, sealed, never mounted into the account;
  the plain original is unlinked, not wiped, and snapshots and backups keep it). `amethystora-agent` wraps an agent whose login is protected in `ame vault run`
  unless the agent's tools are at level 2 with a root mode, where it starts the agent without the login and says why. `apps/vault-service/src/tools.rs`'s list of
  variables that steer programs is the only thing that keeps a secret out of `LD_PRELOAD`: extend it, never shorten it.
- **Sync** is Bitwarden's protocol, written clean-room from `plans/vault-sync/spec-*.md` (never Bitwarden's or Vaultwarden's code), off until connected, only while
  the vault is open, and only through `amethystora-vault-net` (own user, the network, **no key, no state, no home**; a socket only the Vault's user opens). The
  connection and its links are items of the first vault (`SERVICE_ONLY`); the address is in `amethystora-sends` and never in `setup.json`, the manifest or an image's
  `policy/`. **Each kind is one switch** (`kinds` in the connection item, `ame vault sync kind KIND off|on|sealed`, the Sync page; `sync-kind`, approved once, `Export` to send and `Use` to stop, the three
  sentences in `outside.rs` with their `# N_` lines): off, plain (`on`: a Login, a secure note or an SSH key item that Bitwarden's apps open) or sealed (a note only this vault opens). An item's own
  choice wins and choosing its kind's takes it away; a kind covers items set to Ask every time and those added later, but a kind never said for leaves an Ask item off; a non-exportable key never goes. What needs
  libcrypto (an OpenSSH key written, a key or certificate read back) is `sync/convert.rs` and runs in `amethystora-vault-work` (`sync-ssh`, `sync-key`, `sync-certs`), never in this service. **Files** are
  attachments of the item's own note (`sync/attach.rs`, spec-attachments.md): made once, never changed (a changed file is a new cipher and the old one goes to the server's trash), built from the connected address and
  never from a URL a server names (one token, checked), sent only on that origin, refused with a reason before any request on Bitwarden's cloud or without premium, and the one `DELETE` there is, of a slot just opened and
  not filled, is the only one `amethystora-vault-net` lets through. A PUT never names `attachments` or `attachments2`, and change detection compares attachment ids too (Vaultwarden does not move `revisionDate`). A switch that turns a part off or between plain and sealed takes the fields the vault wrote out of a person's own Login and trashes only what the vault made; what a lock leaves half made is `pending` in the connection, cleaned first next run; `sync now accept` sends a lost file again; the machine's `device-identity` certificate goes only by its own choice, never by the Certificates kind.
- **There is no browser extension of its own** (the maintainer's choice of 2026-11-03): Bitwarden's works with the same Vaultwarden or Bitwarden account sync uses, so the Vault gives instructions instead, on the window's **In your browser** page, in `vault.md` and in the agents' knowledge. No native host, no recipe for one, no manifest and no ID of ours in the browser policy; Bitwarden's own IDs stay in its allowlist. `user-setup.hooks.d/29-retire-vault-browser.sh` takes away the manifest an earlier image wrote, only where it is exactly ours. Keep the page a fact and no more: it shows no Bitwarden logo and never claims to be Bitwarden.
- **Clean rooms.** BackMyCodes' formats (`plans/vault-bmc/spec-formats.md`: BMC1 transfers, its export version 2, 2fa.directory's shape) are kept byte for byte and
  nothing of its code or test files is here; Bitwarden's and Vaultwarden's clients, KeePassXC's and the like are never read. EFF's large word list is built in
  (`apps/vault-core/assets`, CC-BY 3.0 US, `usr/share/licenses/eff-wordlists`), and `usr/share/amethystora/vault/2fa-directory.json` is 2fa.directory trimmed at one
  commit by `build_files/shared/trim-2fa-directory.mjs` (MIT, `usr/share/licenses/2fa-directory/SOURCE` says which commit and the sums).
- **The agent never gets in**: `never.rs` refuses every spelling of `ame vault`, `amethystora-vault*`, `org.amethystora.vault`, the sockets and the folder, and no tool reads
  it. `agents/knowledge/vault.md` and the skill say what to tell a person: keep both accurate with the commands.
- **On the desktop**: the window follows Notes' rules (a `gtk::ListView`, every call async, a frame over 16 ms is a bug, one wakeup a second only while the Authenticator
  page shows a code; a command it opens in a terminal is read back the moment it ends, through `process::in_terminal_then` and the
  wrapper's `--done`, never by polling), copies as a secret (`x-kde-passwordManagerHint`, cleared after 30 s), hides a revealed secret after 30 s and on focus loss, sets
  `PR_SET_DUMPABLE` 0 (so its portal helper is the same program started again) and is in `share-guard-apps`. Its icon is the door of `branding/icons/glyphs.mjs`
  in all eight packs, with the tile `generate.mjs` draws (`amethystora-vault.svg`); the logos switch is a marker file (`~/.config/amethystora/vault-logos`) that
  `ame vault logos` writes.
- **Backup and moving**: `ame vault backup` writes `~/.local/share/amethystora/vault-backup/vault-DATE.avb`, the sealed items and the key under the **recovery slot
  alone**, which needs no open vault; `amethystora-backup` runs it before each restic run, seven kept; `ame vault restore FILE` asks for the recovery key. There is one
  recovery key a vault, never split, and nobody (not Amethystora) can make another.
- **Tested** by cargo in the apps stage (`VAULT_VERIFIERS_REQUIRED=1`, with `openssl`, `osslsigncode`, `pdfsig`, `certutil`, `jarsigner`, `ssh` and `oathtool` as separate programs, so
  that no test passes by skipping; also on OpenSSL 4), by `build_files/shared/test-vault.sh` from `20-tests.sh` (the unit, the policy, the sockets, the recipes, the sentences,
  the crates, the data), and by `tests/vm/suites/74-vault.sh` on a booted image (real polkit, SELinux enforcing, a virtual fingerprint reader, the TPM, `ssh`, `pdfsig`,
  a Vaultwarden in a container). **By hand, and not in CI**: BackMyCodes' own app reading what this writes, Bitwarden's own server, Acrobat, Bitwarden's extension,
  a real TPM and reader.

### The Control app

Amethystora Control (`amethystora-control [page [entry]]`, **Control** in the app grid, `Super+Ctrl+Shift+C`) is
the window for Amethystora's own settings outside security: the theme, wallpaper and transition, the
window layout, widgets, the terminal's greeting and tools, games, Android apps, drawing tablets, the AUR,
web apps, the add-ons, the AI agent, the image and stream, developer mode, an image of one's own, the CPU
scheduler, the firmware, freeing space, the pools of disks, starting over, and the setup file. What it shows is one script,
`system_files/shared/usr/libexec/amethystora-control-list`: `ame control` prints it and the window asks
it for `--json`, so a setting added or changed there reaches both. Never write a setting into the window.

- **Every change is an `ame` command.** The script writes each command in plain words, so that the check
  in `20-tests.sh` for commands that do not exist sees them, and a setting no `ame` command reaches gets
  a recipe first. A command marked `terminal: true` opens in `amethystora-in-terminal`; every command
  that asks something, explains a risk or needs a password is marked so. One run directly has no
  terminal and must never prompt.
- **The window runs only what the list gave.** An entry's id and one of the values the list gave pick
  the command the list has for it, which `apps/control/src/backend.rs` runs as an argument vector. The
  only things typed in the window, a slider's number, a new widget's name and a repository to trust, are
  checked there first; the repository against `amethystora-trust-image`'s own expression, which
  `20-tests.sh` compares. The key is picked in a file dialog.
- **The list answers `--json` in under a second** (suite 80's budget): its command substitutions are bash 5.3's
  `${ cmd; }`, which fork nothing (`20-tests.sh` requires bash 5.3), and a slow reader runs once, in the background,
  with `read_in_background`.
- **Security stays in Security,** by the maintainer's rule: nothing in the script or the window runs
  `ame security`. The sidebar opens Security, Updates (whose switch automatic updates keep) and Backups
  (whose controls the backup keeps). The one exception, by the maintainer's choice of 2026-10-18, is the AI
  agent's root mode and lock (`ame agent root`, `ame agent lock`), which govern the agent rather than a
  security feature (see Agentic features).
- **The list logic is `apps/control-core`**, a crate without GTK, which the agent's `change` tool uses too,
  so that both change a setting the same way; Control only says each refusal in its own words.
- **Pools of disks** are the System page's Disks group: a `disks` entry with one row for each pool from
  `amethystora-raid status --json`, each with the actions the list gives it, which `backend.rs` runs with
  `pool(action, id)`. Every one of them is an `ame system raid` command in a terminal (`20-tests.sh`).
- **Reset settings**, before Start over, is `ame system reset-settings` (`/usr/libexec/amethystora-reset-settings`):
  the account's dconf, less its language, keyboard and accessibility, and the machine's `/etc` against
  `/usr/etc`, each saved first for `undo`. It keeps what the machine knows (`KEEP`) and everything Security
  sets: its `SECURITY` list is the watcher's `DRIFT_PATHS`, which `test-reset-settings.sh` holds it to, and a
  unit a security switch enables or masks goes in its `KEEP_UNITS`.

Its manual page is `control.md`, and each card links to its own section. The sidebar draws the same
wordmark as Security, and the app grid's icon is the tile `generate.mjs` draws (`amethystora-control.svg`).

### The Backups app

Amethystora Backups (`amethystora-backups [FOLDER | status]`, **Backups** in the app grid, **Open With**
on a folder in Files) is the home folder as it was at any hour the hourly snapshots
(`amethystora-home-snapshot`) or the daily restic backup (`amethystora-backup`) kept, on one timeline, and
putting back what was lost. What it shows is one script, `/usr/libexec/amethystora-restore`, run as the
user: `status`, `points`, `ls`, `versions`, `put-back` and `open`, each with `--json`. `ame backup
snapshots` and `ame security ransomware restore` take their lists from `points` too, and the security
report's `backup-runs` check reads `status`. Never work out a point, a path or a state in the window.

- **The window passes back only what the helper said.** A point is one `points` gave, a path one `ls` listed
  (or a folder above it, the folder the window was opened on, or the file `--versions=FILE` names, Files' Earlier versions),
  and a way to put back one of
  `keep-both`, `replace` and `skip`; each is an argument of its own. The helper refuses a path that
  leaves the home or starts with `-`, and puts back only inside the home.
- **Back in time** (`past.rs`, a button on Browse, no key): the same list in front of earlier points as windows receding, read by `ls`
  when a walk settles, put back keeping both. Put back and Look stay off until the list of the point shown has been read, and a
  folder is asked about first; leaving the view stops a walk.
- **Earlier versions** (R7-33, Files' scripts menu at version 4, key `earlier-versions`): `amethystora-back-in-time --versions -- FILE` opens
  the versions view on the file; the local snapshots are listed first and the whole answer follows, since restic's `find` over every snapshot held
  the page for minutes. **Keep this one too** is `put-back --keep-both`: it never touches the file. A Look at a version from a backup keeps a read-only
  copy in `~/.cache/amethystora-restore/look`, which `usr/share/user-tmpfiles.d/amethystora-restore.conf` ages out after a day through the account's
  own `systemd-tmpfiles-clean.timer` (`20-tests.sh` holds both; the folders stay writable so that tmpfiles can remove it), and the hourly snapshots
  keep it up to 14 days longer: the manual says so.
- **Nothing deletes.** Neither the helper nor the window runs `restic forget`, `prune`, `rewrite` or
  `key`, restore's `--delete`, or `btrfs subvolume delete`; `20-tests.sh` greps for them. Putting back
  adds what is missing and keeps a file that changed beside the old one (`name (2026-10-04 14.00).ext`)
  unless the window was told otherwise. Looking runs restic with `--no-lock`, so it works against an
  append-only repository; `restic mount` is not used.
- **The backup's controls stay here,** by the maintainer's choice: **Set up** runs `ame backup setup` in
  a terminal, the daily run is `ame backup on|off`, and **Back up now** starts `amethystora-backup.service`
  and follows it in the journal. The hourly snapshots are a security switch: the window shows them and
  opens Security, and never runs `ame security`.
- **The last good backup** is the stamp `amethystora-backup` writes to
  `~/.local/state/amethystora/backup-last-success`. A daily run with none for 7 days is stale, which the
  window and the report's `backup-runs` check say. The security watcher runs as root, sees no account's
  backup, and does not notify about it.
- Déjà Dup is off `amethystora.preinstall`, and `20-tests.sh` keeps it off. An account whose Déjà Dup
  backed it up on a schedule is told once, after it is gone (`user-setup.hooks.d/19-deja-dup.sh`). There is
  no retire hook: a machine that kept Déjà Dup keeps backing up with it.

Its manual section is `security.md#backups`. The sidebar draws the same wordmark as Security; the app grid's icon is the icon packs' (`amethystora-backups`), with the tile `generate.mjs` draws (`amethystora-backups.svg`) for other themes.

### The Help window

Amethystora Help (`amethystora-help [wrong|changed]`, two entries in the Amethystora menu, `plans/ui-ideas/r7/spec-help.md`) is two
pages over two helpers, run as the user. What it shows is theirs, kept in memory while the window is open and never written; it
runs only what they gave. Never work a finding, a fix or a change out in the window (`apps/help`). **The rule for this window: it
says only what it has established, and a fix must not make things worse; where it cannot know, it says less.** The Manual's icon
stays on its launcher until a glyph is picked on a board. R7-36, R7-32 and R7-28 have **no key**: they wrap nothing and cost nothing
closed (two menu entries, a launcher, and on Browse a button); R7-61 is `show-me`.

- **Something's wrong** (`/usr/libexec/amethystora-wrong`, `topics`, `look TOPIC`, `--json`) reads until pressed, says what it found
  in a sentence and names at most one thing to press: a fix that is always an `ame` command it shows (`ame system sound`, `network`,
  `printer`, `power-mode`, `clean`...), never `ame security`, or **Open Security** on a page, which changes nothing.
  `test-wrong.sh` holds each topic to that. A fix that can end what is running (`ame system network reconnect`, `sound restart`) opens
  in a terminal, says what it ends first, and is never offered when nothing is wrong. **No internet** decides from NetworkManager's own
  connectivity first; only when that does not say the internet works it sends the router one ping, and only once that check has failed
  and is on it looks up the name NetworkManager's check fetches (`ConnectivityCheckUri`, `--cache=no`), never `amethystora.org` and
  never while the check is off. A silent router, a route with no gateway and an IPv6-only network are not causes. "Busy" is measured
  over a second from `/proc`, not `ps`'s lifetime average. The window and the manual say the ping and the lookup.
- **What changed** (`/usr/libexec/amethystora-changes`, `list`, `undo ID`, `--json`) has no record of its own: it reads the trash, the
  snapshots through `amethystora-restore` (and each one's dconf file, for the settings in its `SETTINGS` table, none of them a security
  setting or able to hold a command), Flatpak's history (the machine's: "was removed", not "You removed", unless the account's own trash
  says so), `amethystora-uninstall removed`, the deployments, what a reset saved and, while the agent's features and its tools that
  change things are both on, its records, each time. `undo` reads the list again, for the one source of that id, and refuses an id not
  on it. Undo only adds back: nothing replaced (`mv -n -T`), a file only to a place inside the home that is not one that starts
  programs or holds keys (`NOT_PUT_BACK_HERE`; a trash entry's `Path=` is text anyone could write), an app only through
  `amethystora-uninstall undo` (a plain `flatpak install` brings it back empty), an installed app has no Undo, and neither has an app
  the agent installed, whose way back removes. A setting or theme whose value an older point also held is not offered again after
  its Undo. The agent's undoings are records with `of` (`act.rs`) and are not listed. A security line only opens Security, and nothing
  naming the duress feature is listed (`test-changes.sh`). Ids carry file names as percent-encoded bytes. `never.rs` refuses
  `amethystora-changes undo` (a button of the person's, by an id they cannot read); `list` passes.

### Pools of disks

`ame system raid` (`/usr/libexec/amethystora-raid`, run as the user, asking sudo for each step that
needs root) makes btrfs RAID pools from spare disks with one command, `ame system raid create`, and
replaces, adds, opens and forgets them. Its plan, done and tested in the VM lab on 2026-10-20, is
`plans/done/PLAN-raid.md`.

- **btrfs's own RAID, nothing installed.** Every layout is offered, each with its cost said first: two
  and three copies, raid10, raid5 and raid6 with btrfs's own warning, and combined. Metadata is raid1 on
  two disks and raid1c3 from three, never parity.
- **Nothing erases a disk it should not.** A disk is offered only when nothing holds it (mounted, swap,
  LUKS, md, LVM, a member of a btrfs or of a pool that did not mount, zram, read-only), and erased only
  in a terminal after `ERASE` is typed, with the list read again just before: there is no `--yes`, and
  `test-raid.sh` checks the filter and that nothing runs without a terminal.
- **A pool** is a GPT partition per disk named `raid-POOL-N`, LUKS2 under it unless the user said no
  (key in `/etc/cryptsetup-keys.d/raid-POOL.key`, one recovery key in every disk's slot 1), lines in
  `/etc/crypttab` and `/etc/fstab` with `nofail`, and the fstab line marked `x-amethystora-raid`, which
  is how the status knows of a pool that did not mount. A pool that lost a disk is mounted degraded only
  by `ame system raid mount`, never by itself and never in fstab.
- **One source of state:** `amethystora-raid status --json`, which needs no root (sysfs, lsblk,
  findmnt, `btrfs filesystem show` on a mounted pool, and what the services write to
  `/var/lib/amethystora/raid`). Control's Disks group and the check read it; its sentences are each
  reader's own. Security's report has no RAID check, by the maintainer's choice.
- **Watched:** `amethystora-raid-check.timer` (hourly, as root) reads SMART into `disks.json` and
  notifies, with an **Open Control** button, what changed since `last.json`, which is written only once
  somebody was logged in to tell. `amethystora-raid-scrub.timer` scrubs monthly on mains power and
  resumes a scrub a suspend cut short. Both are enabled in every image and skip a machine with nothing to
  watch (`ExecCondition=... present`).
- **The system's own disk** (R7-53, `amethystora-disk-health` and its hourly timer as root, `ame system disk-warning on|off`) reads SMART (`smartctl -n
  standby`) and says once per change that the disk is starting to fail, with the days since the account's last backup. Its switch is the timer: the
  read is the machine's, so there is no key and no line in the setup file. A disk of a mounted pool is left to `amethystora-raid check`, so that no
  disk is two notices' subject; a disk first read with bad sectors has them as its baseline; its serial is kept as a checksum. **It has no
  `ProtectClock=`**: a unit with any device allow rule may be shut out of the disks it does not name, which would leave the warning silently dead, and
  that could not be tried without a machine. `amethystora-raid-check.service` still has it and its `smartctl` is run in the lab by hand, not under the unit: look.
- **The system's disks:** `ame system raid mirror` converts an installer-made multi-disk `single`
  system to raid1 (2a). `mirror DISK` copies a system on one disk, laid out as the installer does it,
  onto a new disk at least as large (2b): an ESP of its own and a firmware entry for its shim, `/boot`
  made the RAID1 `md-boot` (metadata 1.0, so each half reads as ext4 and bootupd finds both ESPs through
  it), the root's LUKS with the same passphrase (its `rd.luks.uuid` written into every boot entry, before
  `/boot` is copied: ostree's `--append-if-missing` skips a key already there) and btrfs raid1. It then
  restarts once with `BootNext` on the new disk; `amethystora-raid-mirror.service` finishes only after a
  start that came from it (`BootCurrent`), and before `boot.mount`, while nothing holds `/boot`: the first
  disk's `/boot` joins the array, its ESP is pointed at it, fstab follows, and `amethystora-raid-boot.service`
  labels the mounted mirror for SELinux. Never on a sealed image.
- **Starting with a disk dead:** 64-btrfs.rules keeps a btrfs with a member missing waiting forever, and
  systemd will not add the policy (#36886). On a system over several disks, `amethystora-raid-boot.service`
  writes the boot menu entry **Start with a disk missing** into `/boot/grub2/custom.cfg`, between
  `amethystora-raid` markers, at every start and again at shutdown after ostree finalized an update: the
  default deployment's entry with `rootflags=...,degraded` and `rd.luks.options=...,nofail`, and a 5 s menu.
  Its `ostree=` stays the `/ostree/boot.N` link ostree wrote, the only kind ostree-prepare-root follows.
  The dracut module `amethystora-degraded` (initramfs only) marks the btrfs ready when, and only when,
  `rootflags` asks for `degraded`, and then has dracut's initqueue wait no longer for every `rd.luks.uuid`
  (crypt's `90-crypt.sh`, which knows no `nofail`): nothing mounts around a disk that is only slow unless a
  person chose that entry. `replace system` puts a new disk in its place (ESP, `md-boot` half and LUKS when mirrored).
- **A disk that comes back** has the old copies of what was written while it was away, which btrfs
  counts as errors: the check and the boot write the disks they see missing to `missing.json`, the status
  calls such a disk `returned` rather than counting errors, and **Put the copies back** scrubs, soft-
  balances, re-adds its `md-boot` half and resets its counters.
- `ame security disk-unlock` and the report's disk-unlock tile take the system's LUKS partitions from
  `amethystora-raid root-luks`, never the first LUKS device on the machine, which may be a pool's. On
  an image that is not sealed it enrols the TPM only with a PIN: there the boot menu and initramfs are
  plain files, and a shell from the boot menu or a look-alike LUKS volume changes neither PCR 7 nor 14.
  The watcher reads the LUKS header into `status.json` (`disk_unlock`), and the report's
  `disk-unlock-pin` check names a key set up without one.

### The transitions

Amethystora Transitions (`amethystora-transitions@arsslensoft.github.io`, written for this image and
kept in `system_files/shared/usr/share/gnome-shell/extensions`, not a submodule) reveals a new theme
or wallpaper with an animation, as swww does on Hyprland. It freezes the screen in GNOME Shell's own
`screenTransition` actor, which makes the background manager swap the wallpaper at once instead of
fading it, and once nothing has changed for 300 ms a GLSL shader opens the picture onto the new
desktop: a `Clutter.Effect` on that actor draws the frozen picture itself through a `Cogl.Pipeline`
(`vfunc_paint_node`, the actor's content texture as layer 0), never `Shell.GLSLEffect`, which GNOME 51 no
longer has. `amethystora-theme` calls `Hold` and `Release` on `/org/amethystora/Transitions` (on
`org.gnome.Shell`) around a theme switch so that it is one transition; a change to a watched
background or interface key starts one by itself, and the Dark Style toggle reaches it through
`screenTransition.run()`. The screen is never held for more than five seconds, and nothing is held
while GNOME's animations are off. Its schema is in `/usr/share/glib-2.0/schemas`, for `ame desktop
transition`. `stage.paint_to_content` took a colour-state argument in GNOME 50 (unchanged in 51): check
that call before adding a GNOME to its `shell-version`, which `20-tests.sh` requires to claim 50 and 51.

### Widgets

Widgets are plugins (see Plugins), since 2026-11-06: the extension that ran them as the account's own code, `amethystora-widgets@arsslensoft.github.io`
(`runner.js`, `protocol.js`, `check.js`), is gone, and nothing is read from `~/.config/amethystora/widgets` or run unsandboxed. A widget is a
plugin whose manifest contributes `bar` or `desktop`; the six of the gallery and `hello` are `/usr/share/amethystora/plugins/amethystora.*`
(the image's own, read-only, added with a click and no administrator, so none may ask for `network`, `files`, `window` or `change`: `20-tests.sh`
and `apps/plugin-host/tests/shipped.rs` hold it), and `plugin-examples/` holds the ones that reach a service for an agent or a person to start
from (`ame plugin new ID EXAMPLE`). `amethystora-widgets` and `ame desktop widgets` stay as other names for `ame plugin` (`/usr/bin/amethystora-widgets`).
The `amethystora-plugins` skill documents the format for agents and `plugins.md` for users. A card on the desktop is drawn by Amethystora UI's
`deskwidgets` module from what the `plugins` module hands it (`desktopPlugins()`), and only while `ame desktop desktop-widgets` is on.

### The logo menu

The Amethystora gem at the left of the top bar and its menu of the system's tools is
`amethystora-logo-menu@arsslensoft.github.io`, written for this image in `system_files` (Apache-2.0), in place
of the Logo Menu submodule, which claimed no GNOME past 50 and shipped sixty-five other makers' logos.
It has no settings: its entries are one table in `extension.js`, each an app by its launcher's ID
(`Shell.AppSystem`, hidden while not installed, never installed from the click) or an argument vector,
never a command line. A middle click toggles the overview. Its labels are gettext in domain `amethystora`
(`po/`), and `20-tests.sh` fails on an app it names that no machine has, on a label a catalog leaves
untranslated, and on a GNOME its `shell-version` does not claim. `tests/vm/suites/26-logo-menu.sh` opens
each entry.

### Amethystora UI

`amethystora-ui@arsslensoft.github.io` (Apache-2.0, in `system_files`) is the desktop's own extension, replacing the
third-party ones one module at a time (`plans/done/PLAN-amethystora-ui.md`). Each module is `modules/<module>.js`, started
on its own by `extension.js` so that one that throws leaves the rest running, with its layout in
`stylesheets/<module>.css`, its keys (if any) in its own schema `org.gnome.shell.extensions.amethystora-ui.<module>`,
and the colours that follow the theme in `/usr/share/amethystora/themed/shell-<module>.css.tpl`, rendered per theme
and loaded by the `theme` module. A module carries only what was on in the image: no preferences window, and a key
only where an `ame` command, Control, the theme or the setup file writes it. The tiling is the exception, by the
maintainer's choice of 2026-10-24: it is to be as customizable as PaperWM was (below).

**It is a clean-room rewrite, and must stay one.** The extensions it replaces are GPL. Their behaviour was written down
as specs by agents who read them (`plans/amethystora-ui/spec-*.md`), and the code is written from those specs and
GNOME's own API only. Never read, copy or paraphrase an upstream extension's code, CSS, shaders or strings into it,
and never paste a GNOME Shell function's body into it (wrap the original instead).
`plans/amethystora-ui/IMPLEMENTERS.md` is the brief, and `plans/amethystora-ui/similarity-check.mjs` compares the code
with the upstreams' (it prints numbers only). Each extension a module replaces is added to
`user-setup.hooks.d/22-retire-extensions.sh`, with its version raised: accounts drop it from their extension lists and
lose its settings (and a cache folder it left), unless they installed it themselves.

**The modules after `connect`** come from the desktop plan in `plans/beyond/`: the maintainer's picks in `PICKS.md`
(2026-10-28, and round 4's on 2026-10-29), a spec per group (`spec-*.md`, `spec-r4-*.md`) and the brief
(`IMPLEMENTERS.md`). The ideas came from a third-party theme the brief says never to open, and no string, key, file, class,
command or page names another maker's product (`20-tests.sh` greps the dock's and present's files for such names). The
brief's rules hold for every one: idle costs nothing (signals and monitors only, connected only while a key asks), every
choice is a key of the module's schema read live, and every key has an `ame desktop` recipe with positional arguments
only, a Control entry, a manual section and, for one a person sets, a line in the setup file's tables (`amethystora-setup`,
`test-setup.sh`). Round 4's VM suites are `*-r4-*.sh` (34, 36, 43, 52, 53, 54, 67).

`MODULES` in `extension.js` is the start order, and the stop order reversed: `theme`, `style`, then `comfort` before
every module that moves something, the glass after the dock and the panel's modules it draws behind, `focus` last.
`20-tests.sh` checks it with greps of the lines `    ['name', Class],` (four spaces, one line each: write a new one
exactly so), each holding only which of a few modules comes first, so a module goes anywhere its own checks allow. Round
4's sit between `bezel` and `focus` in the order `captions`, `access`, `touch`, `present`, `deskwidgets`, `shelf`,
`desks`, `agent`, `quit`, `privacy`, and `power` between `today` and `dock`. `firstlook` comes right after `hints`.
Rounds 5 and 6 add `backupdisk` after `connect`, `bars` after `quick`, `audio` and `brightness` after `power`, `appgrid`
after `dock`, `typing` and `keys` after `emoji`, `windows` and `motion` after `tiling`, and `tools`, `rest` and `living`
after `privacy`; round 7 adds `guide` after `tools` and `found` right before `focus`.

**What keeps the shell up.** A screen lock disables the extension and the unlock enables it again: what must outlive a
lock (Later's and Mute's notifications, pinned captures, walk-away lock's count) is kept at module scope, outside the
class. Every `Gio.Settings` a module makes is kept and `run_dispose()`d on the main thread when it stops: one left to the
collector deadlocked GNOME Shell for good (GJS sweeping it while dconf's worker dispatched a change), and
`build_files/shared/test-settings-disposed.mjs`, run by `20-tests.sh`, refuses one a file makes and never disposes.
GNOME turns an extension on and off asynchronously, so an enable can come before the last disable has finished:
`enable()` stops modules that are still running first, a second `disable()` must not throw, and a deferred callback
(a later, an idle, an async search) checks that what it touches still exists. A shell entry that takes the keyboard
(the emoji picker, the search box) takes the window's input method with it. On close, call
`giveBackTextInput()` (`modules/emoji/paste.js`), or the window types without IBus until it is focused again. Mutter takes one pong for every
ping with the same timestamp, so give each window you ping a timestamp of its own (the force-quit list).
`20-tests.sh` also runs `node --check` on every file of our extensions, since one syntax error stops every module. A layer
put in the window group is lowered by Mutter's restack: put it back on `restacked`. A whole-screen effect is a plain
`Clutter.Effect` that copies each screen as it paints, as the colour filter is: an `OffscreenEffect` keeps its texture
between paints and shows a stale one after a partial redraw (Focus's desaturate fallback still does). Never read a paint
context's redraw-clip region from GJS (`get_extents()`, `get_rectangle()`: the shell dies on signal 11); testing it for null
is safe. `Clutter.Event.add_filter` never sees a press on a Wayland window: Mutter's filter takes it first. A new
`themed/shell-<module>.css.tpl` reaches an existing account only when its theme is applied again: raise `11-theme.sh`'s
version (17 for round 4, 23 for rounds 5 and 6, 25 with round 7, 26 with its second wave). GNOME 51 dropped `St.BoxLayout`'s `vertical`: write `orientation`.

The modules, and what each replaced:

- `theme`: the theme's `shell-*.css`, loaded again on every theme switch (what `amethystora-theme` wrote into the
  extensions' own settings before).
- `style`, the desktop styles (the maintainer's choice of 2026-10-24, from the design canvas of Hyprland-inspired
  looks: every variant, each theme's suggestion as its default, the account free to change it): `islands`, `floating`,
  `hairline`, `capsule`, `terminal`, `segments` and `neon`, the shape of the top bar, menus, quick settings,
  notifications, dialogs and prompts, OSDs, the switcher, buttons, controls, the dock and the windows. Its one key
  (`...amethystora-ui.style` `style`) is written by `amethystora-theme` from `~/.config/amethystora/style` (`ame desktop
  style`, Control) or the theme's `style.theme`; the module only puts `amethystora-style-<style>` on the shell's top
  group and marks the bar's buttons by place. Every rule is in `themed/shell-styles.css.tpl`, the applications' half is the
  shape of every control in one template, `themed/style-base.gtk.css.tpl`, filled by `render_templates` with each style's radii
  (`STYLE_SHAPE` in `lib.sh`) and followed by the style's own lines and glow (`style-<style>.gtk.css.tpl`; floating has none
  and leaves the applications their own corners), which `20-gtk-apps.sh` appends to both stylesheets. The glass takes each pane's corners from the
  surface's own `border-radius` (`radius: null`), so the stylesheet alone shapes a style; only the windows' corners and
  how the bar is glazed (`pane`, `pills`, `solid`, `bare`) are in the module's `STYLES`, which `20-tests.sh` holds to the
  `STYLE_SHAPE`. Floating is the desktop as it was. Each theme also names a font (`font.theme`, `ame desktop font`) and a
  monospace font (`mono.theme`, `ame desktop mono-font`), Fedora's packages or `09-fonts.sh`'s pinned copies from Google
  Fonts. See Colours in applications.
- `tweaks`, Just Perfection: animations at 0.6 through the shell's slow-down factor (never `enable-animations`), no
  workspace popup, no Activities button.
- `workspaces`, Space Bar: the six labels after the logo; rename only in its menu (the six stay six); the keys
  `Super+Ctrl+Alt+Left/Right` swap a workspace with its neighbour.
- `meters`, TopHat: a reading every 5 s that redraws only what changed; the process walk only while a menu is open;
  only interfaces with a device behind them count. A meter hidden in the bars' `layout` key reads nothing (`ame desktop
  top-bar meter`, Control's three switches, which are that key and no key of their own), and with all three hidden and
  the busy chip off there is no timer.
- `tray`, AppIndicator: the shell is the StatusNotifierWatcher and host, from the public SNI and dbusmenu specs.
- `search` and `fuzzy`, Search Light and Fuzzy App Search: `Super+Space`'s box over GNOME's own search, and app search
  that forgives typos, with Parental Controls' filter kept and nothing on disk.
- `screenshot` and `bazaar`, Gradia's and Bazaar's integrations: the drawing tools over GNOME's screenshot bar and
  Extract Text's hand-off to Gradia (`--ocr-file`); Uninstall and Bazaar's page in GNOME's app menu.
- `tiling`, PaperWM: windows side by side on a strip that scrolls to the focused one (`modules/tiling/`, the strip's
  arithmetic apart in `layout.js`). `enabled` in `org.gnome.shell.extensions.amethystora-ui.tiling` is the classic layout
  when false (`ame desktop layout`; an update leaves it as it is). Every other setting the spec names (gaps, focus
  frame, steps, open positions, focus mode, animation time, drift speeds, map, edges, gestures, top-bar icons, window
  rules in `winprops`) is a key of the same schema under PaperWM's own key name and the image's value, read by
  `modules/tiling/config.js` and applied live, and the extension's `prefs.js` (`ame desktop tiling-settings`, Control's
  **Tiling settings**) is the window for them, written fresh from the spec's description of the pages. The 100 keys
  in `...tiling.keybindings` are the spec's, 54 bound as the image ships and the rest empty for whoever wants them,
  the live switcher among them (`live-alt-tab*`, GNOME's window switcher with the window chosen drawn where its strip
  would scroll it, over a fog). With two screens or more each shows a workspace of its own (`modules/tiling/screens.js`; `settings.js`
  turns `workspaces-only-on-primary` off meanwhile, beside `edge-tiling` and `attach-modal-dialogs`, and leaves
  `dynamic-workspaces` the account's: with it on, `screens.js` wraps GNOME's workspace tracker's check so that a
  workspace a screen shows counts as kept alive, which leaves one to spare beyond the screens'. The image's are fixed:
  GNOME Shell's own override sets dynamic ones under `[org.gnome.mutter:GNOME]`, which beats a plain section in a
  session, so the image's are in that section of zz1 too, and `20-tests.sh` reads them with
  `XDG_CURRENT_DESKTOP=GNOME`; `tests/vm/suites/29-screens.sh` runs on four screens): GNOME's active
  workspace is the screen with the keyboard's, the others are drawn as live clones of their windows in a layer over
  the background, the pointer moving to a screen makes its workspace GNOME's, GNOME's slide is skipped but for a
  workspace no screen showed, and a workspace keeps its screen by connector (the screen of GNOME's workspace unplugged,
  the most recent one still shown becomes GNOME's); a workspace has one strip, on the screen showing it (`engine.strip()`
  ignores the monitor there). With one screen none of it runs. `workspace-colors`, `workspace-backgrounds` and
  `default-background` give a workspace a frame colour and a picture of its own (`backgrounds.js`, St widgets over
  GNOME's background that take no clicks); the glass still blurs GNOME's own wallpaper.
  A glide carries the speed the strip already has into the next one (a cubic from the place and speed it starts with,
  timed from the frame last drawn), so that keys pressed in a row and fingers let go never stop and start the strip;
  keep that when touching `engine.js`'s `layout()`. It takes a GNOME shortcut's combination only where one of its own collides with it:
  in memory for the keys it can wrap, and, for the four it cannot (`focus-active-notification`, `shift-overview-up/down`,
  `rotate-video-lock-static`, `TAKE_OUT` in `tiling/keys.js`), by removing the combination in dconf; any other shell or
  media key keeps what it holds (often the person's own choice) and the tiling gives the combination up in memory. It
  writes down what it took in
  `~/.local/state/amethystora/tiling.json` (`taken`), which `amethystora-setup-manifest` reads to keep the account's own value
  and the module gives back when it is off. The file is written with a copy beside it (`tiling.json.bak`) that stands in
  when it is missing or unreadable, and records that do not fit are dropped on load, so a bad file never keeps the
  tiling from starting nor a shortcut from coming back. A strip follows its monitor's connector, and one whose screen
  goes away is merged into the primary's. `21-retire-paperwm.sh` hands an account over from PaperWM once.
- `dock` and `glass`, Dash to Dock and Blur my Shell: the dock is GNOME's own list of pinned apps in a row at the
  bottom (`modules/dock/`), on the main screen unless `screens` says every, as the top bar is unless `every-screen` is
  on (the maintainer's choice of 2026-11-02); the glass the glass is a shader (`modules/glass/lens.glsl`) and a
  stylesheet, with no setting either. Each screen's wallpaper is blurred once into a texture the surfaces share (the top
  bar, the dock, menus, notifications, unfocused windows, the overview), drawn behind a surface by a `Clutter.Effect`
  with its own shader and never by `Shell.GLSLEffect`, which GNOME 51 no longer has (`20-tests.sh` fails on it in any
  extension of the image; the transitions draw their reveal the same way). The glass shows the wallpaper, not the window behind it. A module that
  draws a surface of its own tells the glass through `glassSurfaces()` and `surfacesChanged()`. An effect on an actor
  switches Mutter's culling off for it, so an unfocused window whose whole pane lies under another unfocused glass window
  loses its effect (`glass/windows.js`, recomputed on stacking, focus and size changes, never per frame). The dock's,
  the tiling's and the glass's colours are `/usr/share/amethystora/themed/shell-{dock,tiling,glass}.css.tpl`, from the
  theme's palette like every other module's, with neutral stand-ins in the module's own stylesheet for a theme not yet
  applied (`11-theme.sh` applies it again at version 10, keeping the account's wallpaper); `20-tests.sh` renders each for
  every shipped theme.
  An app's icon is grey while Flatpak updates it (`dock/flatpak.js`, the key `updating`, on), for Bazaar, `flatpak update` and uupd
  alike, none of which says so: read through folder monitors from what a pull leaves on disk, the app's name in the download's
  mountpoint `/var/tmp/flatpak-cache-*/<id>-XXXXXX` for the system helper's path, and in the staging folder's commit for a
  pull straight into a repository, with a check every 5 s only while something is updating, since a dead pull says nothing. It is
  only grey: never `updating`'s refusal to start the app, which an app says of itself.
- `connect`, GSConnect's top bar part: one icon, with the battery, while a paired phone is connected, and a menu to send
  files, send a link, ping and open the texts; the clipboard bridge. It only watches for the program's bus name, so it
  costs nothing while the phone link is off (see The phone link). Caffeine, never switched on, was dropped.
- `power` (on): the battery's health, cycles and heaviest apps in its quick-settings menu (`battery-details`), and a chip
  naming an app that kept the processor at 70 % for two minutes (`busy-app`), at most once an hour, on the meters' own 5 s
  reading; a terminal is never offered Quit.
- The pill's **Later and Mute** (`pill/later.js`, on; `later`, `later-times`): a snoozed notification stays in GNOME's
  list, marked seen, and comes back as GNOME's own; a mute holds an app's banners as quiet does. Everything held is in
  memory, GNOME's Do Not Disturb is never written, and calls, alarms and urgent banners always pass. **What's new**
  (`pill/whatsnew.js`, `whats-new`, see The manual) says an update once, from the version an account saw in
  `~/.local/state/amethystora/whats-new.json`; a new account or a rollback says nothing. **Getting set** (`today/getting.js`,
  `getting-set`) ticks a young account's first steps by what the machine says, never by a click.
- `captions` (off): live captions of what apps play, made on the machine by `/usr/libexec/amethystora-captions` (Fedora's
  `gst-vosk`; nothing in its pipeline writes or sends the sound, which `20-tests.sh` greps for). `ame desktop captions on`
  asks, then downloads Vosk's small English model once, pinned by SHA-256, into `~/.local/share/amethystora/captions`
  (`amethystora-sends` names it from then on). It listens only while an app's stream plays, never while the screen is locked,
  and a valve stops recognition after 2 s of silence; while words show, a full-screen window is composited. The model is
  English only: Dictation (`plans/PLAN-dictation.md`) is not built.
- `access` and `touch` (each part off): colour filters (`filter`, `filter-on`, `filter-strength`), the reading ruler
  (`Super+Ctrl+R`) and read aloud (`Super+Ctrl+S`); touch mode and the pen's ring of tools (`Super+Ctrl+P`, or the pen's
  button once `ame desktop touch pen-button on` set GNOME's stylus action). The board's `Super+Alt+R` and `Super+Alt+S` were
  taken (the tiling's, GNOME's screen reader), and `20-tests.sh` fails if anything else binds the three. The filter is one
  effect on `global.stage`, so that Zoom and every screen go through it, a 3x3 matrix in linear light (Machado, Oliveira and
  Fernandes, 2009) in one pass per screen; another module lays a matrix over it with `access.setExtraMatrix(id, matrix |
  null)`, which Focus's grey uses, so the two multiply rather than fight. An HDR screen, or one the GPU cannot copy, is left
  unfiltered and the person told once; screenshots keep the apps' colours. Read aloud gives each sentence to `spd-say` on
  its standard input, never its command line.
- `present` (offered, off) and the screenshot's **thumbnail** and **pins** (`capture-thumbnail`, `capture-pin`, on):
  presenting keeps the screen awake through gnome-session's idle inhibitor and holds banners through the pill's hold;
  Scribble is `Super+Alt+D`. Up to six captures pinned above the windows, in memory only, blurred while the screen is
  shared or recorded.
- `deskwidgets` (off): see Widgets.
- `shelf` and `desks` (off): the shelf (`ame desktop shelf off|edge|shake`) is the Shelf app's window, which the module
  places, keeps above and on every workspace, and recognizes only by the shelf's own program; the first drop is read by the
  shell and handed to `amethystora-shelf take` on standard input, within limits `20-tests.sh` holds to `model.rs`, items
  stay in memory, and dragging one out offers a copy. A desk (`ame desktop desks`) is a set of screens known by maker,
  model and serial (and Thunderbolt devices): the sound output, the dock's place, a Focus profile (through Focus's own
  `pick()`) and the power mode (power-profiles-daemon's `ActiveProfile`, left alone while a hold is on) come back there,
  with a card the first time and Undo after. `~/.config/amethystora/desks.json` (0600, with `.bak`) never reaches the setup
  file, which carries only the two switches.
- `agent` and `quit` (on): see Agentic features.
- `guide` (`show-me`, on; `ame desktop show-me`): the Manual's `[Show me](guide:ID)` calls `Start(id)` on `/org/amethystora/Guide`, and a
  ring and a card walk the steps of `modules/guide/guides.js`: a name in `guide/targets.js`, or a button of an app by its English
  message, which the app itself looks for on `/org/amethystora/Where` (`apps/common/src/where_is.rs`) in the session's language, from
  the window's corner: GTK's accessibility tree leaves every padding out of its positions, so it cannot say. Any program of the account
  can ask, so the app answers only a name in its own `locale/en.json`, only for a label on a button, and only the part in view.
  The shell asks again every few seconds while a step shows, draws nothing while the app has lost the focus, and brings the Manual (which
  minimizes itself when a guide starts) back whichever way the guide ends. It presses nothing, and a step it cannot find says its words
  and leads back to the page. `check-guides.mjs` holds every guide at build time as `check-manual.js` does links: add a guide with its
  link in the page.
- `privacy`, after the modules it reads (`connect`, `pill`, `quick`, `meters`): the **share guard** (`share-guard`, `blur`
  by default, or `card`) covers the windows of `share-guard-apps` (Notes, Security, Backups, Help, Vault, the password managers) and Files
  in the Private folder while the whole screen is shared, from Mutter's first handle, and uncovers only once Mutter's
  screen cast service says every stream is a window's; the **shield** (`shield`, a tile by default) is described under
  The Security app; **network by app** (`network-apps`, off) lists in the network meter's menu, only while it is open, the
  account's own connections by app, naming an address only from what systemd-resolved already knows (`NO_NETWORK`).
  Walk-away lock is a security switch (The Security app). Beside it, Files' scripts menu has **Copy without hidden data**
  (`24-files-menu.sh` version 2): `/usr/libexec/amethystora-clean-copy -- PATH...` has `mat2` clean a copy in a private
  folder and moves it beside the original as `NAME (clean).EXT` with `mv -n`; paths come from argv only, never
  `NAUTILUS_SCRIPT_*`, and a PDF is cleaned in mat2's light mode, which keeps its text (`mat2`, `perl-Image-ExifTool`).
- `emoji` (`Super+.`): `[org.freedesktop.ibus.panel.emoji]` `hotkey` is empty in `zz1-amethystora-modifications`,
  since IBus reads a text field's keys before Mutter's bindings and would take `Super+.` and `Super+;` for its own panel.
- `firstlook`: Pick a look (`amethystora-control welcome`) is started from a unit by `user-setup.hooks.d/25-first-look.sh`, so
  it has no activation token and opens behind, under the overview and GNOME's welcome dialog of a first login. The
  module activates it once the person is free: at once, or when GNOME's dialog closes. It never does over a window the
  person chose, an app launched from the overview, or the Tour. It is armed only where the hook opens the page (a home
  under a day old with no `first-look.json`), and it knows the page by the hook's command line, which `20-tests.sh` holds.

- `hello` (`greet`, on; `plans/ui-ideas/r7/spec-first.md`), after `firstlook`: a new account's first login says hello in each language Amethystora
  speaks, then goes on. A layer of the shell in `uiGroup`, never a window, so that it changes nothing the first look relies on: it holds the
  keyboard with `pushModal` as a modal dialog does (`SYSTEM_MODAL`), which firstlook's `_free()` already waits for, over GNOME's welcome dialog, and any key,
  click or touch gives it back at once. Who is greeted is the hook's own rule (a home under a day old, no `first-look.json`) with its plain mark `hello.json`.
  The words are pen strokes written in `branding/hello.mjs` (`node branding/hello.mjs`), each a picture of three measures, when the pen gets here, ink and
  glow, read by one small shader whose one uniform a frame sets; add a language there and in the module's `WORDS`, and `test-hello.mjs` holds them to `po/`.
  Reduced motion stands the words still together (loaded small, at 405x189: five pictures stay under the 2 MB a module may hold; each effect lets go of its picture
  with its word). The Soft set's chord is the `sounds` module's, as at every login. The layer is named `amethystoraHello`: the transitions' `_freeze` declines
  while it is up (a theme set under it shows at once, a frozen picture never covers it), it waits for the gem flight's sky (`amethystoraRevealCover`) before it opens, a
  dialog taking the key focus from under it ends it, and `Show()` does nothing while a modal other than the overview is up.
**Round 7's pill** (`plans/ui-ideas/r7/spec-pill.md`; `tests/real/checklist/60-r7-pill.md`; `build_files/shared/test-pill-logic.mjs`). Two at once: `activities.second` sits beside the first as a bubble, a child of the pill since `PanelMenu.Button` lays out only its first child; a call and a recording come first in `ORDER` and keep the place alone (Present too when it is first; something the system puts right after them, like a flat battery, is no `ALONE`) and `MARKS` are never the bubble. An activity may give `short`, `progress` (a ring round its place), `beside: false` and `showing(seen)`: a new activity adds one line to `ORDER` and needs none of them. Every plate motion is two edges from the pill's middle, so that one activity travels as it always did; `pill/spring.js` carries the speed across a change of mind. `pill/moving.js` keeps a suspend inhibitor through a lock in a keeper at module scope, since walking away is the point; every item ages out (`agedOut`) and the hold is given up after twelve hours, so nothing keeps the machine awake for good, and the inhibitor reaches logind as `block-weak`, which the user's own Suspend ignores. `pill/levels.js` (the key `music-bars`, off until a GPU shows it costs under 1% of a core) is `pw-record` on the player's own stream (never another's) in a scope, read from a 150 ms timer, only while the bars are mapped and the screen is not blanked nor the computer on its battery, with `node.dont-fallback` (`test-levels.sh`) and calling itself GNOME's level meter so that it is no microphone. `pill/code.js` copies with `St.Clipboard`, and `Symbol.for('amethystora.clipboard.secret')` names that text for Amethystora Clipboard and the phone link's sending, which refuse it: GJS cannot subclass `Meta.SelectionSource` (it stops the whole extension at import), and `Meta.SelectionSourceMemory` used from JS brought the shell down. The shell reads no text: the phone link's program finds the code (`apps/connect/src/codes.rs`) while `org.amethystora.Connect` `codes` is on and sends the signal `Code` with the digits alone. That key is never in the setup file and is on the never list in every spelling (`ame`, `gsettings`, `dconf`). A bar that moves with sound is set, never eased: an ease asks for every frame.

**Round 7's other groups** (`plans/ui-ideas/r7/spec-<group>.md`, the checks in `tests/real/checklist/60-r7-<group>.md`; where a spec and the
code differ, the code is right). What must outlive a screen lock is at module scope, as above. **Not built:** R7-04 (a command's progress in the
pill): Fedora 45's kitty (0.48) draws OSC 9;4 progress in its own window and tab bar only; the documented way out, `on_program_status_change`
(OSC 7501), is kitty 0.50 (see the later group, below). R7-47 (reading the screen in another language, with Bergamot) was kept out on
2026-11-02 for its 20-minute build stage and its missing Chinese, Japanese and Korean, and was built on 2026-11-06 with the stage moved off the
image: the translator is compiled on the person's machine when they ask (see the later group); Chinese, Japanese and Korean are still out.
- **Find** (`modules/search/`, `/usr/libexec/amethystora-file-find`). *Old words* (R7-49, `old-words`) matches a table of word lists, each one
  gettext message with alternatives split by `|`, to a thing here, and **no string names another maker's product**: of other systems' names only
  `task manager`, `control panel`, `recycle bin` and `print screen` stay, and `test-search-words.mjs` (which reads the four catalogs too) and
  `20-tests.sh` fail on more. It never installs a missing app. *Files by what you remember* (R7-60, `remembered`) sends what its rules read
  (kind, period, size, name) to the helper, a Python process on Tsparql, as JSON on **standard input, never an argument** (an audit log keeps
  command lines); the SPARQL has nothing typed in it. A word alone, more than 16 words, or a date read two ways is no question and the search
  goes on as usual; an installed app whose name starts with what was typed keeps Enter from both providers. **`~/Private` is out of the index**:
  Fedora 45 indexes the whole home, mount included, so `Private` is in localsearch's `ignored-directories` in zz1 (`20-tests.sh`), the query
  leaves it out and the answer refuses it again. No file name goes to a log: a failed open logs an error number only. R7-62 is round 4's
  `windows/hold.js`; only the Emoji row and the board's words were added.
- **Files** (`tools/opener.js`, `tools/room.js`, `bazaar.js`). **`amethystora-flatpak-ask`** installs and removes through a libflatpak
  transaction because `flatpak install -y` and `--noninteractive` both have the system helper ask polkit with interaction off (seen on the bus):
  no password prompt can ever appear. `amethystora-open-anything`, `-uninstall` and `-free-space` use it, and so must any new helper that installs.
  *Anything opens* (R7-40, `open-anything`): `build.sh` runs `open-anything-types.py` after dx, which writes the kinds nothing here opens into
  `zz-amethystora-open-anything.desktop` and runs `update-desktop-database`. **The launcher's `zz-` is load-bearing**: that command sorts a kind's
  list by launcher name and GIO takes the first, so an app added later (a layered package, a derived image) must sort before it; Flatpak's exports
  come first anyway. Off is a hidden entry of the same name in `~/.local/share/applications`, and the card's object leaves the bus. A row says
  whether Flathub verified its maker before the click, and never calls an app free. *Uninstall means gone* (R7-41, `uninstall-takes-data`): the
  data goes to the trash (`Gio.File.trash` of `~/.var/app/ID` and nothing else; never `rmtree` or `--delete-data`, `20-tests.sh`), the app's
  permissions are reset, and `amethystora-uninstall undo` brings the data back (a plain install brings an app back empty; permissions are asked
  again). *Never out of room* (R7-58, `offer-room`) replaces GNOME's low-space notice, found by urgency, transience and its drive icon, only when
  `amethystora-free-space` says a disk is low and 200 MB can go. One click clears only what lies on a low disk: runtimes (the machine's only on a
  machine with one account: one account's `flatpak uninstall --unused` takes a runtime another's own app needs), Homebrew's cache, `podman image
  prune -f` (never `-a`, never volumes), closed Flatpak apps' caches, thumbnails, and trash older than the account's own `old-files-age`, never
  under 30 days and none with housekeeping off. The home part stays in the hourly snapshots for 14 days, so the result is what `statvfs` gained,
  never the estimate. *Earlier versions* (R7-33) is Backups' versions view (see there). R7-29 builds nothing: GNOME's previewer is on the Flatpak
  list and sees the home only, so a file on a USB stick does not preview; widening that is the maintainer's, and `20-tests.sh` keeps the app listed.
- **Sturdy** (`power/flat.js`, `desks/layouts.js`). *A flat battery loses nothing* (R7-54, `flat-battery`) fires only on a fall through 3% during
  this shell's run (a gauge that always reads 0 never closes a working laptop; a UPS is left alone), keeps the open apps and closes through logind.
  **Close now does not save an app's work and must never say it does**; **Not yet** has `flat-hold` (`Super+Shift+N`), bound only while the card
  shows. `flat` comes right after `call` and `recording` in `ORDER` (their controls are only in the pill; `20-tests.sh`, `test-pill-logic.mjs`).
  *Screens come back as you left them* (R7-46, `desk-windows`) keeps in `desks.json` `layouts` app IDs and numbers only, never a title (`20-tests.sh`
  greps `get_title`); `seen` is a counter of visits, not a time, so no habit is on disk; writes go through one queue guarded by the file's etag, and
  the key off by any route deletes the layouts. The update that does not start is R7-52, under The Updates app.
- **Truth** (`tools/reach.js`, `pill/stick.js`, `power/slept.js`, `/usr/libexec/amethystora-sleep-report`): **say less where the machine cannot know**.
  *Is it me or the internet?* (R7-37, `network-reach`) runs only while a Wi-Fi or Wired menu is open: one ping to the router, and the name lookup is
  NetworkManager's own connectivity-check address, read from it and only while that check is on (`20-tests.sh` fails on a host written into
  `reach.js`); a router that drops pings is not blamed, and a connection that is not NetworkManager's primary is asked about the router alone.
  *A USB stick that tells the truth* (R7-38, `removable-writes`, `stick` in `ORDER` after `hello` and before `reading`, held by `test-pill-logic.mjs`):
  per-device dirty data is not readable without root, so **"All on" is said only when a `sync -f` moved no sector of the disk, the machine's dirty
  data has not risen across it, and `amethystora-what-uses` finds nothing holding it; never on a FUSE file system (ntfs-3g's `syncfs` does not
  flush the device), and a sync that hangs or an unreadable input says "Still writing"**. Percentages say "about" and never fall; one `sync` per
  disk, 60 s at most. Its `timeout_add_seconds` ticks (15 s, 30 s when calm, only with a writable removable disk mounted and someone at the
  computer) are the accepted exception to "no timers while idle". *What happened while it slept* (R7-39, `sleep-report`): the module subscribes to
  `PrepareForSleep` at module scope (a lock stops the extension) and reads the battery before and after; the helper runs only on a large loss.
  **It never names the key or mouse the person woke it with**: a wakeup source that fired at or within 10 s of the last wake is left out and no Stop
  is offered for it; with no readable journal it says nothing is known, never "nothing woke it"; USB hubs, controllers, the lid and the power button
  are never named, and the helper itself refuses to stop them. Stopping a device (pkexec, `org.amethystora.wake.policy`) writes
  `/etc/udev/rules.d/90-amethystora-wake.rules`, which the watcher accepts only in the helper's exact line shape; the way back (`ame system
  wake-device`, Control's System page lists each stopped device) never depends on the notice. What was said is salted checksums in `~/.local/state/amethystora/sleep-report.json`.
  The disk warning (R7-53) is under Pools of disks.
- **Devices** (`pill/printing.js`, `pill/through.js`, `quick/sounding.js`, `quick/tv.js`). *Playing now* (R7-57, `sounding-apps`) learns which
  streams are paused from `pactl`, not Gvc (it reports every sink input as invalid), and never listens to a stream's level, so there is no
  "loudest"; a portal's app id is a program's own say-so. *Printing that talks* (R7-59, `printing`; `printing`, then `printing-stuck` after `update`
  in `ORDER`): `com.redhat.PrinterSpooler` signals count only when the bus says root sent them (any account can send them), at most 16 jobs are
  followed, one `ipptool` asks for all of a printer's jobs, polling stops after 10 minutes unchanged, and `jobs-state.test` asks for no job name or
  owner (`20-tests.sh`), so a document's name never reaches the shell; a printer name starting with `-` is refused. `connecting-to-device` is normal,
  not "offline". *Get me through* (R7-45, `get-through`): it **gives everything back whatever happens**: the brightness before and after goes in
  `~/.local/state/amethystora/get-through.json` (plain), put back at the next start if the shell died, UPower's `OnBattery` and the power saver's
  `ProfileReleased` are followed through a lock, and the screen is not dimmed during a call, a recording, a share, or while a microphone or camera is
  in use. *Send to the TV* (R7-43, `send-to-tv`) is offered and off: it installs GNOME Network Displays from Flathub after a click (`tv.js` never
  names flatpak, `20-tests.sh`), and `ame desktop send-to-tv setup` says first what the app reaches (firewalld's configuration, readable without a
  password, NetworkManager, Avahi) and that nothing was seen working with a real TV. **The firewall zone Miracast wants (`P2P-WiFi-Display`) stays
  out of the image**: a new firewall path is Security's, the maintainer's; the tile never says "Sending your screen", which it cannot know.
- **People** (`windows/handover.js`, `windows/simple.js`, `screenshot/cover.js`, `found.js`, `today/year.js`, `/usr/libexec/amethystora-*`).
  *Hand it over* (R7-64, `hand-over`) **is a courtesy, not a lock, and says so**: Mutter's `filter-keybinding` sends keys to the app, Super+L
  (by letter, keycode or the real binding) is the ordinary lock, a lock that does not happen within 3 s gives the window back, and the dialog says that
  an app taking the keyboard can stop the lock key. *Hide what's private* (R7-67, `hide-private`) draws **solid black boxes burnt into the picture,
  never a blur** (`test-cover-private.sh` greps): the picture goes to `amethystora-cover-private` on standard input and to Tesseract through a pipe,
  no file; English only; a street address has no pattern and is not found, which the card says. *A desktop for your parents* (R7-65, `simple-desktop`,
  off) saves each value it sets first in `~/.local/state/amethystora/simple.json` (0600) and a state it cannot write sets nothing; its tiles are
  the default apps or the launchers `simple-tiles` names. *If it's lost* (R7-71, `ame system if-found`, module `found` right before `focus`) writes the line to dconf for the login banner and
  the lock screen: **no code of ours runs on the lock screen** (a plain label is left in `screenShieldGroup`), the label reads the key's default value
  and at most 120 characters, it is never in the setup file and is on the agent's never list, and it is not a security feature. *How do I look and
  sound?* (R7-44) is `amethystora-mirror`, a GTK process with its own scope, never the shell, that writes no frame and ends on close, lock, a share
  or 3 minutes; a camera blocked in Security stays blocked. *Your year* (R7-30, `your-year`) counts days from the birth of `/var` (not
  `/etc/machine-id`, which each deployment copies), claims no "updates that worked" (uupd logs success after any run) and no "threats stopped"
  (the watcher reports), and keeps only the number `your-year-seen`.

**Round 7's second wave, group later** (`plans/ui-ideas/r7/spec-later.md`, checks in `tests/real/checklist/60-r7-later.md`). All offered and off but
the first. *Sign a PDF* is Files' scripts entry over the Vault's `pdf sign`, behind a card saying a reader calls the signer unknown until it
trusts the machine's root (`amethystora-pdf-sign`, hook version 5). *Any language* (R7-47): **the translator is not in the image** (it would add
about twenty minutes to every build, and the image has no compiler): `amethystora-translate engine build` compiles it on the machine when it is
asked for, in a Fedora container of the machine's release pinned by digest, from `usr/share/amethystora/translate/build-engine.sh`, source pinned by
a SHA-256 over the tree; Chinese, Japanese and Korean stay out because their models exceed the 300 MB limit (only the maintainer raises a limit).
*Look good in every call* (`amethystora-camera`, a user unit enabled for nobody) is offered only where its own `bench` says 720p costs under 15% of a
core; the model is PP-HumanSeg (Apache-2.0); no shell module. *Your photos* (`amethystora-photos`, search key `photos`, prefix `photo:`) reads photos
through GdkPixbuf only, on mains power while idle, and keeps its index **sealed in the Vault** (purpose `photo-index`, shards under 1 MiB): a
closed vault means nothing is read or kept. *Another screen* and *Help from someone you trust* are one helper, `amethystora-share-screen`, on
GNOME's remote desktop: a new code every time (through grdctl's standard input), started for the session only and put back when it ends; the port is
the Security switch `remote-screen` (loosening, off, `amethystora-hardening`, the `amethystora-remote-screen` firewalld service in the `amethystora-home` zone
through `HOME_ZONE_SERVICES`), and the commands refuse while it is off or the firewall is not an Amethystora one; the agent may not turn either on or
touch grdctl, the remote-desktop settings or firewall-cmd (`never.rs`); across the internet it cannot work without a relay, and Amethystora runs none. Commands: `ame desktop translate|camera|second-screen|
help-me` and `ame desktop search photos`. Kitty 0.48's OSC 9;4 progress cannot be passed out (R7-04): see the spec.

**Rounds 5 and 6** (`plans/ui-ideas/`, the boards and a `spec-<group>.md` per group; VM suites 51 to 69, one per group).
Every rule above holds for each, and each part is off unless its spec says otherwise:
- **Look** (51): themes made, saved, shared and imported as files (`/usr/libexec/amethystora-theme-file`, `ame desktop
  theme save|import|share|look`, the `.amethystora-theme` and `.amethystora-look` types), refused by `palette.awk`'s
  contrast checks (`checks=1`, held by `control-core`'s `contrast.rs`); drawn wallpapers (`pattern.awk`, `ame desktop
  pattern`), whose Daylight XML is named by the checksum of its list, since GNOME never rereads a slideshow by path; light
  by day (`theme/daynight.js`, `ame desktop day-night`); Atkinson Hyperlegible Next and Mono (`09-fonts.sh`).
- **Bar** (56): the top bar's places, size, position (bottom) and hiding (`bars`, `/usr/libexec/amethystora-bar-layout`,
  `ame desktop top-bar`), a second clock (`ame desktop clock`), `ame desktop animation-speed` (the shell's factor, off as
  `St.Settings.inhibit_animations`), `text-size`, `window-motion` (`motion`) and the light desktop (`comfort`), offered once
  by a notification on a software renderer or under 7 GB of memory (`light-desktop.json`; the lab renders in software).
- **Windows** (57): the drop-down terminal (`Super+Alt+Return`), scratchpad, always on top (`Super+Alt+P`), see-through
  (`Super+Alt`+wheel), the focus frame, sizes and reopen closed (`Super+Alt+T`), one window-menu hook (`windows/menu.js`),
  `ame desktop windows` and `window-settings`.
- **Keys** (58): `/usr/libexec/amethystora-keybindings` (`ame desktop keybindings`, Control's shortcuts page), accents by
  holding a key (`typing`), plain paste (`Super+Alt+V`, `keys`) and shortcuts kept from an app per window.
- **Tools** (59): the colour picker (`colour:` in search), `Super+Alt+M` (the microphone), What is using this?
  (`/usr/libexec/amethystora-what-uses`, Files' menu at version 4, the bus `/org/amethystora/Tools`), conversions in Files
  (`/usr/libexec/amethystora-convert`, GdkPixbuf's glycin), clipboard text actions and QR codes (`qrencode`), hidden while
  the screen is shared.
- **Apps** (61): the app grid's order, hidden apps, names and pictures as `appgrid` keys, never a launcher
  (`/usr/libexec/amethystora-app-look`, `ame desktop app` and `app-grid`); Starting with you
  (`/usr/libexec/amethystora-startup`, `ame apps startup`), whose off is the `[Desktop Entry]`+`Hidden=true` shape the watcher
  records quietly; tidy downloads (`amethystora-tidy-downloads`, its own trash for an exact undo); the backup disk notice
  (`backupdisk`); `28-templates.sh`.
- **Sound** (62): clear voice (PipeWire's webrtc echo-cancel, a passive source), presets (filter-chain, smart filter), pause
  when headphones come off, more sound sets, devices' batteries (`power/devices.js`), all `/usr/libexec/amethystora-sound`.
  External screens' brightness (`brightness`, `ame desktop screen-brightness`): `08-hardening.sh` takes ddcutil's own udev
  rule and modules-load file out, since an i2c bus of the display reaches other chips of the graphics card; turning it on
  copies the image's rule from `/usr/share/amethystora/ddc` into `/etc` after saying so, and the agent may never turn it on.
- **Phone** (64): see The phone link. **Restart** (65): see The Updates app.
- **Time** (66): calls in the pill (`pill/meeting.js`, `/usr/libexec/amethystora-meeting-link`, Evolution Data Server's
  calendars), remind me and times anywhere in search, kitty's notice when a long command ends (`ame desktop command-done`,
  `28-kitty-options.sh`) and the missing-command offer (`/usr/libexec/amethystora-missing-command`, off, from an index
  `build-missing-command-index.sh` writes at build).
- **Finish** (68): the digest's lists, the dock's arrivals count, dock size and glass strength, workspace names in the
  overview, the widget gallery (`amethystora-widgets add`), the resting screen (`rest`), living wallpapers (`living`) and
  Trim for screen recordings (`/usr/bin/amethystora-trim`, `apps/trim`; Fedora 45's `ffmpeg-free` has the encoders).
- **Later** (69): captions in ten languages (each model pinned in `amethystora-captions`' `MODELS`) and dictation
  (`ame desktop dictation`), bookmarks, notes and files by meaning in search (`amethystora-bookmarks`,
  `amethystora-file-meaning`, the agent tools' `files-index`), click with the keyboard (`amethystora-letters`, `access`),
  and guest login, a Security switch (`ame security guest-login`, `amethystora-hardening`'s `guest-login`, authselect's
  `with-guest-login` through `/usr/libexec/amethystora-guest`), at GDM only.

### The phone link

`amethystora-connect` (`/usr/libexec/amethystora-connect`, the crate `apps/connect`, a user service
`amethystora-connect.service` that nobody has enabled) speaks the KDE Connect protocol, so the KDE Connect app on an
Android phone pairs with it; it replaced GSConnect (GPL, a submodule and `nautilus-gsconnect` before), and keeps nothing
of its pairings (`user-setup.hooks.d/21-phone-link.sh` turns the service on for an account that had GSConnect on, and
`amethystora-phone-link-notice` says once to pair again). `plans/done/PLAN-amethystora-connect.md` has the decisions; the specs
in `plans/amethystora-connect/` are the protocol, the plugins and the desktop side as facts.

- **Clean room, and it must stay one.** GSConnect, KDE Connect and Valent are GPL. The program is written from the specs
  (`plans/amethystora-connect/spec-*.md`, by writers who read the upstream and wrote down the protocol and what a person
  sees, never code) and from GLib's, GIO's and the crates' own documentation. Never open, copy or paraphrase their code,
  strings or comments into it; wire names (packet types, field names, ports) are the protocol and are used as they are.
  `plans/amethystora-connect/IMPLEMENTERS.md` is the brief.
- **Three parts, one contract**: the program (glib and gio, no GTK, owns the session bus name `org.amethystora.Connect`),
  the app `amethystora-phone` (`apps/phone`, app id `org.amethystora.Phone`: the phones, pairing, the send dialog, texts
  and the reply dialog) and the `connect` module of Amethystora UI. The contract is `apps/connect/dbus/org.amethystora.Connect.xml`;
  the app and the module are its clients and work out nothing themselves. The program raises its own notifications
  (pairing with the verification key, a phone's notifications with Reply, a file received, a call), so that a fault in the
  shell never loses a pairing prompt.
- **Off costs nothing and it is home only.** `ame desktop phone on|off` enables or disables the user unit (Control's
  **Phone pairing**); the program listens only while it runs, announces itself only on interfaces in the `amethystora-home`
  zone (it asks firewalld, read-only, and counts "cannot ask" as not home), and firewalld's `kdeconnect` service (1714 to
  1764) is open in that zone alone. `allow.d/listeners` names its ports for the watcher. Nothing runs as root. The
  unit has no setting that needs a namespace (`ProtectSystem=`, `ReadWritePaths=`, `Private*=`, `CapabilityBoundingSet=`
  and the like; `20-tests.sh` refuses them): a user's manager makes one only as a user namespace of the program's own,
  from which `/proc/PID/exe` of GNOME Shell and the Phone app is unreadable, and the program then refuses both as strangers.
- **What it holds and shows.** Its certificate, key and the paired phones are in `~/.config/amethystora/connect` (0700, files
  0600); the only way a pairing is ever removed is Unpair. A pairing is never accepted without the person, with the phone's
  name and the key in front of them. Text from a phone is shown as text, never markup; a file goes to `~/Downloads/Phone/<name>`
  under a name that cannot leave it and never overwrites; texts live in memory only. Received links are shown, never opened.
- **The clipboard is two switches, both off** (`org.amethystora.Connect` `clipboard-send` and `clipboard-receive`, `ame
  desktop phone clipboard send|receive on|off`, two Control entries): a phone pasting into a password field is a risk the
  person takes on knowingly. Not in the first version: the phone's storage, remote input, run command, sending this
  computer's notifications, and `tel:`/`sms:` handlers.
- **Ring, music and the clicker** (`plans/ui-ideas/spec-phone.md`): ringing the phone (`Ring`, the menu, `ame desktop phone
  ring`) is always offered. `music` and `clicker` (`org.amethystora.Connect`, both off; `ame desktop phone music|clicker
  on|off`) change what the program lists (`identity::advertised`), so a switch closes each phone's connection and it comes
  back with the new lists. Music: the phone's players are the device property `Players`, shown by the pill through
  `connect/players.js`, and the program reads the session's MPRIS players only while a phone takes them (`players.rs`), no
  album art either way. The clicker lists only `kdeconnect.presenter`, never `kdeconnect.mousepad.request`, which would offer
  the phone's remote input; that packet is heard only as Next, Previous or Blank (`media::clicker_of`), only while a module
  connection said `Presenting(true)`, and becomes the signal `Clicker`, which Present presses as Page Down, Page Up or B.
- **Other computers pair only once chosen** (`org.amethystora.Connect` `computers`, off; `ame desktop phone computers
  on|off`, Control's **Pair with other computers**), for files and the clipboard: off, a device that says it is a computer
  is neither shown nor let in, and its transfers are cancelled (`apps/connect/tests/computers.rs`).
  `user-setup.hooks.d/26-phone-computers.sh` turns it on once for an account that had already paired one.
- **Interop** is tested against Fedora's `kdeconnectd` as the phone (`apps/connect/tests/`), and a real phone last; what a
  real phone does differently goes back into the specs.

### Plugins

`plans/PLAN-plugins.md` is the design, every decision in it final; **phase 1 is built** (the root helper, the store, the host, the
sandbox, the proxy, `ame plugin`), and nothing of the shell, Control, Security, the watcher, the broker's `state`/`control`/`events`
or the widgets' move onto it is. **A plugin's code never runs in the shell**: it runs in bubblewrap as a process of its own and prints
what to show, which the host reads, refuses or cleans, and the shell will draw from a fixed set of elements. `apps/plugin-host` is one
crate (no GTK; only the host's `service.rs` links GLib) with two programs: `amethystora-plugin-root` and `amethystora-plugin-host`.

- **The store is root's** (`/var/lib/amethystora/plugins`, tmpfiles): `store/<tree digest>/`, `accounts/<uid>/grants.json` and
  `history.jsonl`, `quarantine/<ticket>` (0700, cleared at boot and after an hour). `amethystora-plugin-root` is its one writer and
  starts only through pkexec, one polkit action per verb in `org.amethystora.plugins.policy`, picked by `exec.argv1` (a new verb is a
  new action, and `test-plugins.sh` counts ten). The account is `PKEXEC_UID`, never an argument; each argument is held to
  `valid_perm_arg`'s short set of characters (so `$(command_line)` in the dialog is what runs); what is granted is checked against the
  manifest in the *frozen* files, never against what the caller says. `prepare` reads a bounded tar on stdin (`tree.rs`: files and
  folders only, no link, device, FIFO, `..`, absolute path or setuid bit; its own modes; `openat` with `O_NOFOLLOW|O_EXCL`), `inspect`
  prints what was frozen as tab-separated lines for a person to read, `commit ID DIGEST PERM...` moves the frozen tree into the store and
  rewrites `grants.json` atomically (a block and the old digest, kept as `previous`, survive an update), `grant` widens an installed
  plugin, `commit-shipped` grants exactly a manifest of `/usr/share/amethystora/plugins` (never `network`, `files`, `window` or
  `change`), and `revoke`, `block`, `remove`, `unblock`, `rollback` change grants only (a rollback never gives more than the older
  version's manifest asked). `50-amethystora-plugins.rules` lets the active local session through for `prepare`, `inspect`, `revoke`,
  `block`, `remove` and `commit-shipped`, and names **nothing that gives more** (`commit`, `grant`, `unblock`, `rollback`): those are
  `auth_admin`, no `_keep`, no `yes`. Never `NOPASSWD`, never a `--yes` on `grant` or `add` (both ask in a terminal, a permission at
  a time, `need_terminal`).
- **One definition of everything** (`manifest.rs`, `output.rs`, `sandbox.rs`): a permission is a normal-form string
  (`network=api.github.com`, `files=~/Documents/ci:ro`, `state=clock`, `notify`...), the same text in the manifest check, the helper's
  arguments, the grants and the dialog. The manifest asks with `needs`: `network` entries with `host`, `why` and an optional byte
  `limit` (named hosts, wildcards `*.example.org`, never an address or `any`), `files` (a `~/` path with no dot-folder, no `..`, not
  `Private`, checked again by `files.rs` on the real path after links), `state`/`control`/`events`/`change` as `{names, why}`, and
  `notify|open|clipboard-write|pick|window` as `{why}`; no `why`, no permission. `amethystora.` ids are the system's, so only a
  shipped manifest parses with `parse_with(.., true)`. `network-local` parses and is granted but the proxy gives it nothing yet.
  What a plugin prints is one JSON object (or plain text): the widget format plus `progress`, `until`, `private` and a `card` of at most
  20 elements; icons are theme names, never a path; actions are ids of the manifest's `actions`; text loses control and bidi
  characters; output over 64 KiB or a card over 20 is refused, not truncated.
- **The sandbox is one function**, `sandbox::bwrap_args`, printed by `amethystora-plugin-host sandbox ID` and `ame plugin show` and run by
  `host::Runner::run` under `systemd-run --user --scope` in `amethystora-plugins.slice` (so that the user manager's OOM line names the plugins, not `app.slice`, and the host logs whose limit it was) with `MemoryMax=256M` and `MemorySwapMax=0` (swap is not memory to systemd, and zram let a plugin fill 1 GB), `OOMPolicy=continue` (a scope that fails with `oom-kill` makes GNOME's housekeeping say "Device memory is nearly full"), `CPUQuota=25%`, `TasksMax=64`, 30 s a run, 5 s between
  runs and never two at once, 8 MiB of state (also while a run writes: `RLIMIT_FSIZE` and a check every half second). The filter is written at build time by `apps/plugin-host/seccomp/gen.c` (libseccomp, which the apps stage
  installs) into `/usr/share/amethystora/plugins/seccomp.bpf` and passed on descriptor 9; a missing filter means nothing runs. The host
  loads a plugin only if its folder still has the approved digest (`Store::verify`), and reads grants.json again for every run and
  every proxy connection, so a revoke or block applies at once.
- **The network is a proxy** (`proxy.rs`): the sandbox has only loopback, so a plugin with a `network=` permission gets a socket
  bound at `/proxy/proxy.sock` and `amethystora-plugin-host shim` (started inside the sandbox) forwards `HTTPS_PROXY`'s connections to
  it. It takes `CONNECT host:443` for granted, unblocked hosts, resolves the name itself and refuses if any address is private,
  loopback, link-local or reserved (`forbidden_ip`, IPv4-in-IPv6 judged by the IPv4 it carries), counts bytes sent per allowed entry, so a wildcard's hosts share one allowance (default
  1 MiB an hour), blocks and cuts the connection past the limit, and a block set by the person cuts open connections within a
  quarter of a second. Every refusal is the same 403. At most 16 connections a plugin. A limit block is also written to the store through `pkexec amethystora-plugin-root block`, untested until phase 0.
- **The host** (`org.amethystora.Plugins`, `dbus/org.amethystora.Plugins.xml`, `amethystora-plugin-host.service`, enabled for nobody) runs
  while a plugin is on: `ame plugin on` writes `~/.local/state/amethystora/plugins/enabled`, enables the unit, and the service watches
  that folder and ends itself when the list is empty. Its unit has no namespace setting for the reason the phone link's has none
  (`/proc/PID/exe` of the callers), and callers are GNOME Shell, Control and Security by that file. `/etc/amethystora/plugins.conf`
  (`ALLOW=off|granted|all`, `BLOCK_NETWORK=yes`; unreadable or writable by others counts as the strictest) is read by the host.
- **The agent** never grants: `never.rs` refuses `ame plugin grant|unblock|add|update|rollback|dev`, `amethystora-plugins` with those
  words, `amethystora-plugin-root` and `org.amethystora.plugins` in any spelling; it may `revoke`, `block`, `on`, `off`, `check`,
  `list`, `show`.
- **Tests:** cargo (manifest, output, tar refusals, proxy address rules, the helper's flow), `build_files/shared/test-plugins.sh` (from
  `20-tests.sh`), and `tests/vm/suites/72-plugins.sh` with `tests/vm/fixtures/hostile` (a hostile plugin and a hostile account; the suite
  was written and has not been run). Phases 2 and 3 are built (2026-11-06, round 7's plugins group): see the next section. Not built yet: Control's settings form for a plugin
  (`ame plugin set` is the way), Security's section and report check, `plugin-approval`, the watcher and `amethystora-sends`, `pick`, `change`, `events`,
  `network-local`, panels, Files' menu, keybindings and the agent's tools from a plugin.

### Plugins in the shell

`modules/plugins.js` (Amethystora UI, right after `pill`, key `show` of `org.gnome.shell.extensions.amethystora-ui.plugins`, `ame desktop plugins on|off`,
on) is the one client of the host: it watches the bus name `org.amethystora.Plugins` (nothing else while no plugin is on, and with the key off not even
that), believes only the connection whose `/proc/PID/exe` is `/usr/libexec/amethystora-plugin-host` and only its signals, and draws what `List` and
`Updated` hand it from the fixed set of elements, **clamped again** (`modules/plugins/clamp.js`, run under node by `build_files/shared/test-plugins-clamp.mjs`):
text only, a theme icon's name, 20 elements, no plate. The places are the manifest's `contributes`, each with the options the shell reads
(`manifest.rs` `surface_options`, nothing else): the pill's `extra` activity (`plugins/pill.js`, `ORDER` after `saving` and before `quiet`; one activity for
all, the latest change in the pill and the rest rows of its card, countdown counted by the shell while shown, `private` hidden while the screen is shared),
the bar (`bar.js`, status area `amethystora-plugin-ID`, layout id `plugin-ID` in `bars/arrange.js`), a tile (`tile.js`, `_amethystoraQuickId`, added with
`quickSettings.menu.addItem`), search (`search/plugins.js`: the plugin's last output for its prefix; what is typed never reaches a sandbox), a dock badge
(`dock/launcher.js` `setPlugin`, a number in `text` is the count) and the desktop (`deskwidgets.js`). `hidden: true` hides only the pill, the bar and the
desktop; a tile and a search answer stay. Every card ends with the plugin's name, what it may reach and Pause, drawn by the shell.

**The host** (`apps/plugin-host`) gained: surfaces in the manifest, `author` and `settings` (every one is sealed with `vault-client` under purpose
`plugin-<16 hex of sha256(id)>` and reaches the sandbox on descriptor 10, `secrets.rs`; a closed Vault makes the plugin wait, never pause), drafts
(`~/.local/share/amethystora/plugin-drafts/ID`: run with nothing granted, with `sample.json` shown when the run finds nothing to reach, `ame plugin draft|keep|
discard|make|change|fix|new|examples|try|settings|set|open`), the broker (`broker.rs`: a socket per plugin that may ask for anything, `/broker/broker.sock` and
the host's own binary as `plugin` inside the sandbox: `state clock|disk|media|do-not-disturb`, `control media|do-not-disturb` and `open`/`clipboard` only in a
run that a click started (an action), `notify` once a minute; a draft is answered from samples), and the rules that pause a plugin by itself (five failed runs in
a row, three that hit 30 seconds, two over their memory, more than 30 results a minute; the `Paused` signal says it once with its reason; the shell offers Ask the
agent to fix it while the features are on). `ame pill start|update|end` (`/usr/libexec/amethystora-pill`, group `pill`) is the account's own script live in the
pill with no sandbox: one file per pill in `$XDG_RUNTIME_DIR/amethystora/pill`, watched by `plugins/script.js` (plain files of a few bytes only, a waiter on the
script's process takes it away), and the agent may run it. Control lists everything as `rows` (`plugins`, `plugins-agent` "Made by the agent", `plugins-shipped`,
`plugins-show`); `ame plugin keep` and `open` are on `never.rs`. A debug build of the host honours `AMETHYSTORA_PLUGIN_TEST_IMAGE` (a folder with `shipped/`,
`store-root/`, `seccomp.bpf`, `plugins.conf`) so that it can be tried without an image; a release build has no such seam.

### The icon packs

`amethystora-glass`, `-glass-light`, `-neon`, `-neon-light`, `-duotone`, `-line`, `-clay` and `-muted`
(`system_files/shared/usr/share/icons`, Apache-2.0) are drawn in code by `node branding/icons.mjs` from the glyphs in `branding/icons/glyphs.mjs`, each icon in colours of its own
(the maintainer's choice of 2026-10-23, from the design canvas: Neon, Glass and Duotone; the other five are the choice of
2026-10-25, from the theming board: packs that read on every ground a theme can have, and one for each style; Pixel, drawn then, was dropped on 2026-10-27). Glass is
the default (`DEFAULT_ICON_THEME` in `lib.sh`, and `icon-theme` in `zz1-amethystora-modifications`, which `20-tests.sh`
compares), Glass for light follows a light theme, and each theme names the pack that suits its colours and style in its
`icons.theme` (Catppuccin Mocha and Gruvbox Duotone, Latte and Rosé Pine Dawn Clay, Everforest and Nord Muted, Matte Black
Line, Tokyo Night Neon). The "for light" packs deepen every colour of a ramp in OKLab lightness until it reads at 3:1 on a
light ground, Line and Clay hold theirs in a band that reads on the grounds of all ten themes (`band()` in `icons.mjs`:
measure a pack's ramps, never judge them by eye), and
`icon_theme()` in `lib.sh` writes the account's own `~/.config/amethystora/icons`
(`ame desktop icons`, Control's Appearance page) before the theme's `icons.theme`, passing over a name that is not installed
(candy-icons, which an update removed, so no account is left on a theme the image lacks). They cover Amethystora's apps, GNOME's, the system's, folders, file types (a language's by its letters, `label-PY`),
devices, dialogs and generic pictograms, and other makers' apps by what each does, as candy-icons covered them (the
maintainer's choice of 2026-10-24), but **never another maker's logo**: an app whose icon is its maker's mark (Firefox,
Thunderbird, Steam, VSCodium, LibreOffice...) keeps the icon it ships, bare, on no tile (the maintainer's choice of
2026-10-23, after a tile behind them was proposed), and the script refuses a name `LOGOS` matches. Keep an Amethystora
app's glyph its own: another maker's backup or notes app gets a glyph of its own. To add an icon, draw a glyph (two inks:
the body, and the details over it, never a hole cut into it; `line()` draws a straight line along an axis as a bar, since
a box with no width takes no gradient), add a line to `SET` with its ramp and the names it answers to, and run the script;
`20-tests.sh` holds the eight packs to the same number of icons, no symbolic icons and no links, and the icon of every `amethystora-*.desktop` launcher to a file in each; suite 27 asks GTK itself and counts only the pack's own file.

### Colours in applications

`/usr/lib/amethystora/theme/palette.awk` (called through `derive_palette` in `lib.sh`, from `render_templates`, so that every
template has its keys) works out of a theme's `colors.toml` the colours an interface needs: the surfaces (`window_bg`,
`view_bg`, `headerbar_bg`, `sidebar_bg`, `card_bg`, `popover_bg`, `dialog_bg`), the accent as a fill with a label on it
(`accent_bg`, `accent_fg`) and as text (`accent_text`), the same for a destructive button and the status colours
(`success|warning|error_text`, `_bg`, `_fg`), the line a control is drawn with (`outline`, 3:1), the dimmed label's strength
(`dim_opacity`, `dim_alpha`, 4.5:1) and six blocks for the segments style, each moved in OKLab lightness only, so the hue
stays the theme's. `test-theme-palette.sh` (run by `20-tests.sh`) measures every one of them, on every surface, by an
implementation of its own. A palette whose slots 2, 3 and 1 are not a green, a yellow and a red (Matte Black's, Rosé Pine
Dawn's) adds `success`, `warning` and `error` to its `colors.toml`. Where the values go:

- **libadwaita and GTK 4** (`themed/palette-gtk4.css.tpl`): libadwaita's colour variables on `:root`, written at the top of
  `~/.config/gtk-4.0/gtk.css` by `20-gtk-apps.sh` and wrapped in `@media (prefers-color-scheme: <the theme's>) and (not (prefers-contrast: more))`,
  which GTK 4.22 honours in a user stylesheet and switches live, so High Contrast keeps libadwaita's own colours, and an
  app drawn in the other scheme than the theme's (the Dark Style switch) keeps them too. It replaces the import of
  the Sweet GTK 4 stylesheet the two Amethystora themes had (Sweet stays their GTK 3 theme). GTK does not reload `gtk.css`
  while an app runs: open apps take a new theme or style at their next start (the notification says so), the shell, the
  Amethystora apps and kitty at once. **GTK 3** (`palette-gtk3.css.tpl`) takes adw-gtk3's named colours, which GTK 3 cannot
  put behind a query, so the hook leaves them out while `high-contrast` is on; `:root` and `var()` are GTK 4 only and
  GTK 3 rejects them, which is why the style files have none.
- **The shell**: `shell-glass.css.tpl` and the styles draw the accent as `accent_bg`/`accent_fg`, never `-st-accent-color`
  (GNOME's nearest of nine), so that apps and shell agree; the search box, Bazaar's Uninstall choices and the clipboard menu
  have templates of their own (`shell-search|bazaar|clipboard.css.tpl`, a `#uiGroup` more specific than the extensions' own
  stylesheets). The lock screen is the shell's, which runs no extension by design, and stays Amethystora's brand like the login.
- **The styles' shapes in the applications**: `style-base.gtk.css.tpl` (windows, dialogs, controls, a linked group rounded at
  its ends, split buttons, cards, the end rows of a list, popovers, tiles, switches, tooltips, toasts, troughs). A selector
  in it must beat the generic `button:not(.circular)…` rule (0,3,1) of the same file, which is why a split button's names
  `:not(.circular)`; GTK takes a value from the highest-priority provider whatever the selector, so only this file's own
  specificity matters. The hairline and terminal files draw controls with `outline`, and the terminal keeps a radio button
  round. To see a style's look, render a libadwaita app with the rendered stylesheet (the board of 2026-10-25 did, in a Fedora
  container with `adwaita-1-demo` under Xvfb); the build only checks that the files render whole.
- **The Amethystora apps** read neither GTK stylesheet of the desktop: `palette.rs` reads `theme/palette.toml` (the colours
  above, from `themed/palette.toml.tpl`), `current/style.name` and `theme/shapes.conf` (every style's radii, written by
  `render_templates`), and `shape.rs` scales every `border-radius` of their stylesheets by the style's card radius over the
  16 px of their own cards (`--radius`): a pill stays a pill except in the terminal style, and a square box drawn round (an
  avatar, a dot, a knob: a radius of half its side) stays a circle in every style.
- **Fonts**: `font.theme` for the interface and documents (a size up), `mono.theme` for `monospace-font-name`, kitty
  (`font_family` in the rendered `kitty-theme.conf`) and the terminal style's top bar. Fira Code and Geist Mono are Fedora's
  packages (Fira Code's carries no licence file, so `09-fonts.sh` pins its OFL.txt), DM Mono and Victor Mono are pinned copies.
- **The pointer**: `cursor.theme` names Halo (`amethystora-pointer-halo`, white) on a dark theme and Ink
  (`amethystora-pointer-ink`, dark) on a light one; Facet is the third set. `node branding/cursors.mjs` draws them and
  packs the Xcursor files itself (byte for byte what `xcursorgen` writes), one file per drawing, and writes each drawing
  as SVG in `cursors_scalable/<name>/` too, which is all GNOME Shell 51 reads for its own pointers (`st-cursor.c`; X11
  and GTK 3 still read the Xcursor files); every other name is a line of the set's `aliases`, which `10-icons.sh` makes
  into links in both (a layer would store copies in full). Flatpak binds
  the host's `/usr/share/icons` into every sandbox, so they follow there. `apply_gnome` writes `pointer_theme` (the
  account's `ame desktop pointer`, else the theme's) on every switch, over only a value an image wrote. Breeze stays
  installed and named by no theme. Shake to find and the larger pointer on dense screens are Amethystora UI's
  `pointer` module.
- **Other programs**: `10-vscodium.sh` merges `themed/vscodium.json.tpl` into VSCodium's settings over Default Dark or Light
  Modern (a `vscode.theme` still selects a theme of its own; a theme extension copied into the extensions folder is not read
  by current VSCodium without an entry of its own, which is why the colours are settings); `30-qt-colors.sh` writes
  `themed/kdeglobals.tpl`'s groups into `~/.config/kdeglobals` unless the file's colours are somebody else's (no
  `AmethystoraTheme` key). GtkSourceView schemes are not written: the Flatpak Text Editor cannot see the host's.

### Packages from other distributions

`amethystora-pkg` (`/usr/bin/amethystora-pkg`, a bash script; `amepkg` is the link `17-cleanup.sh`
makes, because the build's rsync skips links in `system_files`) makes distrobox containers from
templates and drives each with its own package manager. It reimplements Vanilla OS's Apx feature set
from a written description, clean-room: never read or copy Vanilla OS projects' source, help text or
artwork, and never ship their definitions or community templates. No command, menu, app or string
names Vanilla OS, Apx, VSO or Sideload; the manual says once, in `software.md` (and once in each of its translations), that `import` reads
Apx's YAML files, and `20-tests.sh` fails on any other mention. `ame pkg` runs it too, under `ame`,
the command over Amethystora's own; `amy` is a Homebrew formula, so nothing may take that name.

- Built-in templates and managers are JSON in `system_files/shared/usr/share/amethystora/pkg` (the AUR's
  `paru` and `arch-aur` under `aur/`, shown only while `~/.config/amethystora/aur` exists, which `ame
  apps aur` writes); the user's own are in `~/.local/share/amethystora/pkg`. Every built-in template
  image is pinned by digest, the pins shipping in the signed image; Renovate follows each tag's digest
  (custom manager, depType `pkg-template`) and never moves a template to a new release, which is done
  by hand with its `description`. Images are pulled by digest before `distrobox create`, so neither the
  pin nor `policy.json` can be skipped. Leave the policy's catch-all as it is, and do not re-sign third-
  party images under `ghcr.io/arsslensoft`. `import` shows what a definition runs and asks before keeping
  it (`--yes` skips that): a pin chosen by a template's author proves nothing about the image. A
  template's packages are names, and the AUR packages a paru template names are left to `install`.
- `policy.json` checks `registry.opensuse.org/opensuse/leap` against openSUSE's two container keys
  (`keyPaths`, containers/image 5.33 and later), both from openSUSE's `openSUSE-build-key` package,
  whose RPM signature was checked against openSUSE's project key: `opensuse-container-key.pub`, the
  2048-bit key that signs openSUSE's published images today, converted to PEM from the package's
  OpenPGP file, and `opensuse-container-key-2023.pub`, its successor, as the package ships it. The rule
  covers Leap only, by the maintainer's choice: the rest of openSUSE's namespace holds build projects
  (infrastructure, tools, templates, Factory's staging) that sign with keys of their own. Every update
  is verified against the same file, so `20-tests.sh` has skopeo load it.
- Containers carry `amethystora.pkg.*` labels and real names `pkg-<name>`; distrobox's own
  `manager=distrobox` label stays, since distrobox, DistroShelf and Ptyxis find containers by it. Only
  labelled containers are listed or touched.
- `--json` field names are documented in `software.md#in-scripts`: keep them stable.
- AUR packages are built only after their PKGBUILD has been shown on a terminal, `--yes` or not, and are
  never upgraded unattended; the AUR's own metadata is shown first.
- `amethystora-pkg-upgrade.timer` (user, enabled with `--global`) upgrades the containers daily and,
  when its timer starts it (`$TRIGGER_UNIT`), skips a run when uupd's hardware checks would fail, with
  the thresholds of `/etc/uupd/config.json`. uupd's own distrobox module stays off there.
- A `.deb` or `.rpm` opens in `amethystora-pkg-install.desktop`, the default for both types in
  `/etc/xdg/mimeapps.list`: a ClamAV scan, provenance, and Flathub or the container's repositories
  offered first. The security report's two-week check on the containers is in
  `amethystora-security-status`, like every check.

Everything it installs is fetched on the user's machine from its source (DistroShelf from Flathub,
images from their registries, packages inside containers), so `NOTICE` and `SOURCES` do not change for
it.

### Windows programs

An `.exe`, `.msi`, `.lnk`, `.bat` or Microsoft Store package opens in `amethystora-windows-program.desktop`, the
default in `/etc/xdg/mimeapps.list` for every type shared-mime-info gives one (a modern `.exe` is
`application/vnd.microsoft.portable-executable`): `/usr/libexec/amethystora-windows-program`, in a terminal, scans it
as `amepkg install` does, names the signer from its Authenticode signature (`osslsigncode verify`) only when the
signature checks out, refuses a file changed after signing, and offers what
`/usr/share/amethystora/windows-programs.json` names for its file name before the Windows program: the app from
Flathub, a launcher in the image, an `ame` command (Steam's), alternatives, the web version as a web app. Last,
Bottles from Flathub, granted X11 for the account alone (`flatpak override --user`), the file handed over through
the document portal (`--file-forwarding`), and a first bottle made with `bottles-cli new`. `.dll`, `.sys`, `.efi`
and the like share the PE type and are said not to be programs.

- **The map is written here** (Apache-2.0), from public sources: never copy Zorin's `zorin-exec-guard` or its
  `app_db.json` (GPL-3.0). An entry's fields are `name`, `match` (a regular expression for the file name, matched
  without regard to case), `flatpak`, `launcher`, `ame`, `alternatives`, `web`, `wine: false` (known not to run)
  and `driver: true`. `20-tests.sh` holds it to those fields and compiles every pattern, and
  `check-windows-programs.yml` asks Flathub's API for every ID, weekly and when the map changes.
- Not taken: WinApps (AGPL, and files with no licence at all), WinBoat (the manual names it as something to
  install) and Wine in the image (Bottles brings its own runners).

### New machines

- **How long it takes to start** (`/usr/libexec/amethystora-start-times`, `ame system start-times`, Control's System page; R7-51). `amethystora-start-times.service`
  is `Type=exec` and waits for the start to finish with `systemctl is-system-running --wait`: no timer and no polling. It records `FinishTimestampMonotonic` less each
  disk's unlock (the journal's start and finish of `systemd-cryptsetup@*`), and nothing on a soft reboot, into `/var/lib/amethystora/start-times/seconds.tsv`: a date and
  seconds, never a time of day, and keep it to that. The file is `root:wheel 0640` in a 0750 folder (a calendar of a machine's use is the administrators' alone; an account outside
  wheel gets no card, and an unreadable file never says "nothing measured"), the first line and the rest of its week are kept when it is trimmed (1000 lines at most), a write that does not
  complete keeps the file as it was, and a stamp in `/run` keeps a restart of the unit from measuring a boot twice. The unlock is found by the journal's two message ids, an index
  lookup, never a unit pattern. It reports what it measured, also when it looks bad: "Still as fast as day one" only where the oldest home folder shows measuring began in the machine's first
  week (a machine updated into this says "as fast as when it began to be measured" and since when), "a little more slowly" past 10 % and 1 s, "more slowly" (the doctor's `slow-start`,
  so Something's wrong too) past a quarter and 3 s. What changed (the update it came with, what took longest, the disk's unlock and the splash left out) is worked out only when asked
  (`start-times why`, Control's button, the doctor when slower), never in `--fields`, which the Control list waits on, and never stored.
- `user-setup.hooks.d/17-welcome.sh` opens `amethystora-manual welcome` once per account, through
  `systemd-run --user`, because the setup service kills what it leaves behind. The Manual itself
  spawns nothing. Ninety seconds later, `/usr/libexec/amethystora-layout-offer` offers classic windows
  in a notification, which only `ame desktop layout classic` acts on. The classic layout is the tiling
  turned off in its own key, which an update leaves as it is.
- `amethystora-gpu-check.timer` offers the image whose driver drives the machine's NVIDIA card (PCI class
  `0x03`, vendor `0x10de`, by device ID, see Drivers), or the image without the driver on one with no card
  NVIDIA's driver drives, once per machine, through `amethystora-notify-users --action` and `ame system rebase IMAGE STREAM`. It
  never switches by itself; its state is `/var/lib/amethystora/gpu-check`.
- The hourly snapshots of the home folders are on from the first boot of a machine installed from an
  Amethystora ISO: both kickstarts leave `/var/lib/amethystora/new-installation`, which
  `system-setup.hooks.d/26-snapshots-new-installation.sh` acts on and removes. A machine updated into it
  keeps them as its owner left them.
- `amethystora-backup` stores `amethystora-setup-manifest`'s JSON beside each backup, as a restic
  snapshot tagged `amethystora-setup` (no secrets in it), and `ame restore-setup`
  (`/usr/libexec/amethystora-restore-setup`) makes the setup again from it, each step chosen first.
  Anything that can add a snapshot can write that manifest, an append-only repository's compromised
  client included, so nothing in it runs unread: it never runs the Brewfile it keeps (Ruby), installs
  Homebrew's names only, asks before each tap and each Flatpak remote the machine lacks, and checks every
  name as `ame setup` does. Its loops read on fd 3, so that what they run keeps the terminal to ask on.

### Your setup as a file

`ame setup save|diff|apply` (`/usr/libexec/amethystora-setup`, the `setup.just` group) keeps the
account's setup in `~/.config/amethystora/setup.json`: what `amethystora-setup-manifest` writes, less
the machine and everything that is an address or a script (the Flatpak remotes, the Brewfile, templates
and package managers of the account's own, the AUR's switch). A file can be shared, so it names things
only. Its settings are a fixed short list, and only where the account changed them from the image: the
layout, the dock's apps, keybindings by schema and key (the image's own custom keybindings by id, and
only their binding), and the tiling's settings in `tiling` (the script's `TILING_KEYS`, each checked against its
type and range; never `enabled`, the window rules, a picture's path or a workspace's name or folder; test-setup.sh holds the table to the
schema). Never add a GNOME key that can hold a command, or a field that
is an address. The script refuses a file with a key or a name it does not know, `save` included;
`apply` shows what it adds and sets and asks first (`--yes` skips that), and takes nothing away without
`--remove`, which always asks in a terminal. `build_files/shared/test-setup.sh`, run by `20-tests.sh`
against stand-ins for flatpak, brew and GNOME, checks all of this. The manual page is `setup.md`.

### An image of your own

`arsslensoft/amethystora-image-template`, a repository of its own, builds an image `FROM` the stable image,
signed with its owner's cosign key. Its build runs two scripts shipped here, so keep their arguments as
they are: `/usr/libexec/amethystora-trust-image` (`ame system trust-image` on a machine, before it
switches) writes the owner's key, a `registries.d` file and a `sigstoreSigned` policy entry for the
image's repository, and refuses Amethystora's own repositories; `/usr/libexec/amethystora-image-check`
fails a build that loosened the signature policy, carries Fedora's logo or release package, or lost the
`ame` commands, and `20-tests.sh` runs it on every Amethystora image. The image's policy takes a
repository it does not name unsigned (Fedora's `insecureAcceptAnything` for `docker`), which is why the
trust comes first. `ame system rebase` must keep taking signed images only. The manual's section is
`developer.md#an-image-of-your-own`.

The template's `build_files/files.sh` makes its `setup.json`, `branding/` and `policy/` part of the image with
three more scripts shipped here, so keep their arguments too (`plans/done/PLAN-image-editor.md`):
- `amethystora-setup bake FILE [--root DIR]` takes the file `ame setup save` writes as every machine's and
  account's defaults, through what the image already reads: `preinstall.d/image-setup.preinstall` (Flathub's
  apps only), `zz2-image-setup.gschema.override` (the dock, the extensions, keybindings, layout and tiling, the
  image's own extensions always kept on), compiled into each extension's own schemas as `build-gnome-extensions.sh`
  does, a dconf file for the image's custom keybindings, and `/usr/share/amethystora/image-setup/{theme,Brewfile}`,
  which `11-theme.sh` and `18-image-brewfile.sh` read for a new account. It fails on what the image lacks.
- `amethystora-brand DIR [--root ROOT]` sets the name (`PRETTY_NAME` "NAME, built on Amethystora", `VARIANT`;
  `ID=amethystora` stays, which bootc-image-builder and the installers go by), the wallpaper
  (`image-setup/backgrounds`, which `amethystora-theme` lists before every theme's own, and `zz3-image-branding`),
  the logo (About's and GDM's Fedora-named files), the login picture, through
  `/usr/libexec/amethystora-login-background`, which `12-login-screen.sh` uses for Amethystora's own, and the look
  (`image-setup/{style,icons,font,mono-font}`, which `theme/lib.sh` reads after the account's own choice and before
  the theme's). `amethystora-image-check` takes a Fedora-named logo only as Amethystora's picture or the owner's.
- `amethystora-image-policy DIR [--root ROOT]` takes `policy/`: `agent.conf`, `security` (a profile, then
  `KEY=VALUE` switches through `amethystora-security-config`), `hardening` (only its `BUILD_SAFE_ON` and
  `BUILD_SAFE_OFF` switches, through `amethystora-hardening set`, which a build can apply), `desktop.json` (its
  `DESKTOP_KEYS` table of GNOME settings into `zz4-image-policy`, each lockable through a dconf file and lock, and
  image extensions turned off), `updates` (`uupd.timer` on or off and a drop-in for when), and the company's
  network, none of which may hold a secret, since an image can be read by anyone who can pull it: `proxy` (GNOME's
  proxy, lockable, and `environment.d`; host and port, never a login), `dns` (`amethystora-dns on custom`, at
  build only), `flatpak-remotes/` (`/etc/flatpak/remotes.d`, https, the key checked against `NAME.fingerprint`;
  `--only flatpak-remotes` runs before `bake`, which then takes apps from them) and `vpn/` (NetworkManager
  keyfiles of an installed plugin or WireGuard, 0600, without a password, key, key file or `[vpn-secrets]`, and
  their CA certificates in `/etc/pki/amethystora-vpn`). The website's editor
  reads `DESKTOP_KEYS`, `BUILD_SAFE_*`, the hardening and security helpers' tables of words and `TILING_KEYS` out of
  the scripts (`npm run sync:image-editor`): keep their shapes, or change the reader with them.
`test-setup.sh`, `test-brand.sh` and `test-image-policy.sh` check all three against stand-ins in `20-tests.sh`. The template's `build-iso.yml` builds
an owner's two ISOs from this repository's `iso/` at a pinned commit: keep `@IMAGE@` in `iso.toml` the image's whole
repository, and the live ISO's sealed installer for `ghcr.io/arsslensoft` images only. The editor that writes the files
is `/build` on the website (`../amethystora-website`, `src/lib/image-editor.mjs`).

### Sealed images

`:stable-sealed` is each of the six stable images sealed (`Containerfile.sealed`, after Fedora's sealed
test images, https://github.com/travier/fedora-atomic-desktops-sealed, CC0-1.0): a UKI signed with
Amethystora's Secure Boot key carries the composefs digest of the whole tree in its kernel command
line, and the initramfs mounts the system only with fs-verity required. Experimental and for new
installations only: bootc cannot move an ostree deployment to the composefs backend or back, so the
ostree images stay the default.

- **Built in the stable build job**, right after `just build-ghcr`: `just seal` builds `FROM` the local
  image, before it is rechunked. `build_files/sealed/00-sealed.sh` takes out rpm-ostree (with
  `rpm -e --nodeps`: tuned requires it on a bootc system, and dnf would take tuned-ppd with it), bootupd
  and GRUB, installs systemd-boot, copies `system_files/sealed`, sets `image-tag` to `stable-sealed`,
  ships the certificate as `/usr/share/amethystora/secure-boot/amethystora-uki.der`, rebuilds the
  initramfs with dracut's `bootc` module and the image's own `amethystora-sealed`, and writes `SOURCES`
  again. chunkah rechunks the tree **before** the digest: nothing may change the tree after `bootc
  container ukify` (`10-uki.sh`) has read it, and the UKI under `/boot/EFI/Linux` is the only thing
  added afterwards. `10-uki.sh` builds the UKI unsigned; `15-sign-uki.sh` signs it and systemd-boot in
  the `signer` stage, a pinned Fedora image with only sbsigntools added, the one place the Secure Boot
  key is mounted. Pin Fedora's image from `registry.fedoraproject.org` (the Justfile's `fedora_image`
  too): quay.io stops serving a digest of `fedora:44` as soon as Fedora moves the tag, which it does
  daily, and that broke sealing in October 2026. `20-tests-sealed.sh` checks the signatures, the
  digest and the command line. The UKI's kernel arguments are the image's `kargs.d`,
  there is no other way to add one, and they never include a debug shell.
- **Keys: shim and MOK** (the maintainer's choice of 2026-10-04, over replacing the firmware's PK, KEK
  and db). The private key is the `SECUREBOOT_KEY` secret, separate from the cosign key; its certificate
  is `secure-boot.crt`, which `just seal` checks the key against. Until both exist, and always for pull
  requests, `just seal` signs with a throwaway key and labels the image
  `org.amethystora.secure-boot-key=throwaway`, which CI never publishes. Never replace the certificate:
  every sealed machine trusts that one only.
- **Published** with gzip layers (the composefs backend cannot pull zstd:chunked yet, bootc-dev/bootc#2408),
  pushed twice, signed with cosign and attested, then removed from the runner before the rechunk needs
  the disk. It has no SBOM of its own. A failure fails a pull request, but never keeps the stable image
  from publishing (`continue-on-error`).
- **Installed** only by `iso/live/amethystora-install-sealed`, in the live ISO, since Anaconda cannot
  install the composefs backend: a whole disk, a 2 GiB ESP, a discoverable root partition (LUKS2, btrfs,
  subvolume `root`), `bootc install to-filesystem --composefs-backend`, then shim placed in
  `EFI/amethystora` in front of systemd-boot (`grubx64.efi`), a firmware boot entry, and every
  `/usr/share/amethystora/secure-boot/*.der` of the image queued with `mokutil`.
- **In the image**, a sealed machine is told apart by `/usr/share/amethystora/sealed`. It has no
  rpm-ostree, so read deployments through `/usr/libexec/amethystora-deployments`, which prints
  `rpm-ostree status --json`'s shape on both kinds of image (on a sealed one from `bootc status`,
  through pkexec and `50-amethystora-deployments.rules`), and branch on the marker for anything that
  changes kernel arguments: they cannot change there. `amethystora-signed-updates.service` does not run
  on it. The manual's `updates.md#sealed-images` describes it for users.
- **`amethystora-sealed` is what makes the seal hold; keep it.** bootc's initramfs does not by itself
  insist on fs-verity for the image the signed command line names; the details stay out of the
  repository until bootc has a fix.
  The module's `amethystora-sealed-verity.service` runs after `bootc-root-setup.service` and halts
  unless every composefs mount has `verity=require` (never the emergency target: its locked root
  account lets the boot continue). An install made by a kernel without fs-verity is refused the same
  way. Drop it only once bootc enforces this itself, and not before every machine's repository has
  fs-verity on `meta.json`, or they stop starting.
- **`systemd.import_credentials=no`** is in the sealed `kargs.d`: credentials from SMBIOS, a hypervisor
  or a TPM-encrypted file on the ESP could add a drop-in to any unit, the guard's included (shown in
  the VM with SMBIOS), and systemd's own units take root's password, SSH keys, udev rules and files
  from them. Plain ESP credentials are already ignored. Nothing in the image uses credentials.
- **Verified in a VM on 2026-10-10**, with the release key, QEMU and OVMF holding Microsoft's keys and
  the image's certificates in MokList: the firmware's fallback path starts shim, shim systemd-boot and
  the UKI; Secure Boot enabled, lockdown on, `composefs=` in `/proc/cmdline`, the root mount
  `verity=require`; a changed object reads as an I/O error; the same disk without our certificate stops
  at shim's Security Violation; the guard halts a disk whose image file lost fs-verity, also when an
  SMBIOS drop-in tries to switch it off (2026-10-11, with `systemd.import_credentials=no`); and `bootc
  switch` applies `policy.json` (an unsigned image under a `sigstoreSigned` rule is refused). Fedora's
  shim in the image carries both Microsoft's 2011 and 2023 signatures. **Not verified:** the MOK
  manager's enrolment screens, LUKS and the TPM, uupd updating a sealed machine, and real hardware.

### COPR Package Installation

COPR packages use the `copr_install_isolated()` helper function from `build_files/shared/copr-helpers.sh`:
```bash
# Install packages from COPR with isolated repo enablement
copr_install_isolated "ublue-os/staging" package1 package2

```

This function:
1. Enables the COPR repo
2. Immediately disables it
3. Installs packages with `--enablerepo` flag to prevent repo conflicts

### Making Package Changes
1. Edit the appropriate shell script in `build_files/base/` or `build_files/dx/`
2. Add packages to the appropriate array (`FEDORA_PACKAGES` or `COPR_PACKAGES`)
3. For version-specific packages, add them in the Fedora version case statement
4. Validate shell script syntax: `bash -n build_files/base/04-packages.sh`
5. Run pre-commit hooks: `pre-commit run --all-files`
6. Test with container build if making critical changes

### Package Security Model
**CRITICAL**: Packages are split into separate arrays to prevent COPR repos from injecting malicious versions of Fedora packages:
- Fedora packages are installed first in bulk, from Fedora's repositories by name (`--repo=fedora --repo=updates`, and `updates-testing` on beta): the akmods Copr and negativo17's `fedora-multimedia`, which outranks Fedora's, are still enabled at that point
- COPR packages are installed individually with isolated repo enablement
- Every third-party repository's signing key is pinned by fingerprint: `pin_repo_key` (`copr-helpers.sh`, with the Copr projects' fingerprints in its table) fetches the key a repository file names, refuses any other, keeps it in `/etc/pki/rpm-gpg` and points the file at that copy, so that dnf never takes a key from the server; `20-tests.sh` fails on a pinned repository that still names one. A new repository or Copr project gets its fingerprint there
- RPM Fusion's release packages come from volunteer mirrors, and dnf checks no signature on a package given as a file or URL: `rpmfusion_release_install` takes RPM Fusion's keys from Fedora's signed `distribution-gpg-keys` and installs the release packages only with a valid signature
- Downloads outside a package manager are pinned by SHA-256 in the script that fetches them (Starship, Sweet, the
  themes' fonts from Google Fonts at one commit in `09-fonts.sh`), the apps' crates by `apps/Cargo.lock`'s checksums, the agent's embedding model by its revision and SHA-256 in `build-apps.sh` (its licences in `usr/share/licenses/potion-base-8M`, named in `NOTICE`), and Flathub is added from the base image's own `flathub.flatpakrepo` after its key is checked

### Licences and trademarks

The repository is Apache-2.0, and the image is not: it redistributes other people's work on their
terms. `NOTICE` is where each of those is named, and `20-tests.sh` checks what follows.

- **A file under another licence says so.** Name it in `NOTICE` and ship its licence in
  `system_files/shared/usr/share/licenses/<project>/`. The zsh configuration is Prezto's and zimfw's
  (MIT); `50-usb-realtek-net.rules` is GPL-2.0 and says so in its first lines.
- **Credits stay as their authors wrote them.** The udev rules name who wrote each and where
  (`ublue-os/config/pull/...`); `debrand.py` leaves such comment lines alone, and so should an edit.
- **Licence texts stay in the image.** Do not delete a package's documentation without first moving
  its licence, copyright and NOTICE files, as `05-override-install.sh` does.
- **Nobody else's logo.** Not in the icon packs (`branding/icons.mjs`), not in the top bar, not from a setup
  hook. Fedora's guidelines ask a remix to carry neither
  `fedora-logos` nor `fedora-release`, and to describe itself without Fedora's name: say that
  Amethystora is built on Fedora, never that it is a Fedora desktop.
- **Source travels with what asks for it.** A package from RPM Fusion, negativo17 or Copr under the
  GPL, LGPL, MPL, EPL or CDDL has its source package fetched into the image by `source-manifest.sh`.
  The kernel modules built outside any repository (xpadneo, xone) only warn there, so `20-tests.sh`
  requires their source packages.
- **A package's own gaps are ours to fill.** Falco's leaves out the licences of the libraries
  compiled into it: they ship in `usr/share/licenses/falco/bundled`, at the versions its release pins,
  and NOTICE names them. Its probe is passed on under MIT (`driver-MIT.txt`).
- **A file the ISOs change says so.** `iso/live/build.sh` marks each Anaconda file it edits, and NOTICE
  names the changes, as it does for GNOME Tour, fprintd and Solaar.

## Configuration Files

### Key Configuration Locations
- `system_files/shared/` - System-wide configurations
- `build_files/base/` - Base image build scripts
- `build_files/dx/` - Developer experience build scripts
- `build_files/shared/` - Common build utilities
- `.github/workflows/` - CI/CD pipeline definitions

### Linting/Build Configurations
- `.pre-commit-config.yaml` - Pre-commit hook configuration
- `Justfile` - Build recipe definitions and validation
- `.github/renovate.json5` - Automated dependency updates
- `Containerfile` - Container build instructions

## Build System Deep Dive

### Justfile Structure
The `Justfile` is the central build orchestration tool with these key recipes:

**Validation Recipes:**
- `just check` - Validates Just syntax across all .just files
- `just fix` - Auto-formats Just files
- `just validate <image> <tag> <flavor>` - Validates image/tag/flavor combinations

**Build Recipes:**
- `just build <image> <tag> <flavor>` - Main build command (calls build.sh)
- `just build-ghcr <image> <tag> <flavor>` - Build for GHCR (GitHub Container Registry)
- `just rechunk <image> <tag> <flavor>` - Rechunk image for optimization
- `just seal <image> <tag> <flavor>` - The sealed variant of a built image, `localhost/<image>:<tag>-sealed`
- `just secure-boot-key` - Makes the sealed images' Secure Boot key and `secure-boot.crt`, once

**Image/Tag Definitions:**
```bash
images: amethystora, amethystora-dx
flavors: main, nvidia-open, nvidia-lts
tags: stable, latest, beta
```

**Version Detection:**
- `just fedora_version <image> <tag> <flavor>` - Dynamically detects Fedora version from upstream base images
- For `stable`: Checks `quay.io/fedora/fedora-coreos:stable` (CoreOS does not use cosign)
- For `latest`/`beta`/`gts`: Checks `ghcr.io/ublue-os/base-main:<tag>`
- A `kernel_pin` overrides the result, taking the version from the `fcNN` portion of the kernel
- Returns the Fedora major version (e.g., 43, 44)

Do not assume the streams sit on different Fedora releases. They frequently
resolve to the same major version, so anything version-dependent must key off
the value this recipe returns rather than off the stream name.

### Containerfile Multi-Stage Build
The `Containerfile` uses a multi-stage build process:

1. **Stage `ctx`** (FROM scratch): Copies all build context (system_files, build_files, packaging, etc.)
2. **Stage `fprintd`** (FROM the same pinned silverblue-main): `build_files/shared/build-fprintd.sh` builds
   `packaging/fprintd` with rpmbuild into `/out`. Its build tools never reach the image: `base` mounts `/out`
   for `build.sh`, and `05-override-install.sh` installs fprintd and fprintd-pam from it in place of
   Fedora's, refusing a base whose fprintd is past the spec's, and puts the source package in
   `/usr/src/amethystora`
3. **Stage `apps`** (FROM the same pinned silverblue-main): `build_files/shared/build-apps.sh` installs
   Fedora's Rust, downloads the agent's embedding model (potion-base-8M, at one Hugging Face revision, each
   file checked against its SHA-256), runs the tests of every crate in `apps/` and builds the one program,
   `amethystora-sensor` and the agent's two tools into `/out`, with the licences of their crates, then clears
   its build away. `base` mounts the whole stage at `/apps-stage`:
   `13-apps.sh` installs from its `/out`, and `20-tests.sh` runs its Node. Neither Rust nor Node reaches
   the image
4. **Stage `base`** (FROM silverblue-main): Base Amethystora image
   - Mounts build context from `ctx` stage
   - Runs in three steps: `/ctx/build_files/shared/prepare.sh`, then
     `03-install-kernel-akmods.sh`, the only step the kernel module signing key (`AKMODS_PRIVKEY`,
     and for a throwaway key `AKMODS_CERT`) is mounted for, before any of the build's own
     third-party repositories is added, then `/ctx/build_files/shared/build.sh`, which runs the rest
5. **dx**: not a stage of its own. `just build amethystora-dx` passes `IMAGE_FLAVOR=dx`, and
   `build.sh` runs `build-dx.sh` inside the `base` stage once the rest has finished

**Build Arguments:**
- `AKMODS_DIGEST`, `AKMODS_ZFS_DIGEST`, `AKMODS_NVIDIA_DIGEST` - The kernel and module images, resolved to a digest and checked with cosign by the Justfile once; step 03 copies them by that digest
- `BASE_IMAGE_CREATED` - When the base image was built (seconds since the epoch), which `00-image-info.sh` writes into `image-info.json` as `base-created` for the report's image-age check
- `BASE_IMAGE_NAME` - Upstream base (silverblue/kinoite)
- `BASE_IMAGE_SHA` - Digest of the base image, resolved from `image-versions.yml`
- `FEDORA_MAJOR_VERSION` - Dynamically set by Just (43/44)
- `IMAGE_NAME` - Target image name (amethystora/amethystora-dx)
- `KERNEL` - Pinned kernel version (optional)
- `UBLUE_IMAGE_TAG` - Stream tag (stable/latest/beta)

### Base Image Pinning

`image-versions.yml` is the single source of truth for pinned upstream digests,
and every entry in it is load-bearing. `just build` reads each digest with `yq`
and passes it into the Containerfile, which builds `FROM image:tag@digest`.

Base image entries are keyed by Fedora major version and must be named
`<base_image_name>-main-<fedora_version>`, for example `silverblue-main-44`.
The key is the version `just fedora_version` resolves at build time, not the
stream name, so the pinned digest can never disagree with the version the
kernel and akmods were resolved for.

At a Fedora rollover, add a new entry before building, and remove the one no
stream resolves to any more. A missing entry fails the build with `No digest
pinned for silverblue-main-<version>` rather than silently falling back to a
floating tag.

A pinned base never moves by itself, and most packages come with it.
`bump-base-images.yml` proposes the new digests daily (Renovate's
`image-versions.yml` manager does the same), `20-tests.sh` warns when the base
is more than 10 days older than the build, and the report says how old the base
of the running image is.

`repo:tag@sha256:...` is valid for buildah `FROM` and for cosign, but `skopeo
inspect` and `podman manifest inspect` both reject it with "Docker references
with both a tag and digest are currently not supported". Do not "fix" a working
`FROM` on the basis of that error.

### Publishing and Image Signing

Every tag is pushed twice. podman does not send layer annotations on the first
push unless the layers already exist in the registry, so the first push of a
rechunked image produces a different manifest digest than later pushes. Pushing
each tag once left every tag on its own digest while only one of them received
the cosign signature, the SBOM and the attestation. See
[containers/podman#27796](https://github.com/containers/podman/issues/27796)
and the equivalent workaround in `ublue-os/aurora`. Do not collapse the
duplicate push.

Publishing happens only from main (`PUBLISH` in `reusable-build.yml`, see
Workflow Architecture), so PR builds and dispatches from other branches validate
the image but never push. `Latest Images` publishes only on `merge_group` and
`workflow_dispatch`; there is no cron. Only `Stable Images` is scheduled
(Tuesdays 01:00 UTC). Merging directly to `main` and bypassing the merge queue
therefore publishes nothing.

The `main` ruleset asks of everyone but the repository's administrators, who bypass it: a pull request,
an approval from a code owner, the `Checks` workflow's three jobs, and the merge queue, which squashes.

### Build Script Execution Order
The build runs the scripts in `build_files/base/` in this order, which is not the numerical one:
`build_files/shared/prepare.sh` the first, `03-install-kernel-akmods.sh` as a step of its own, and
`build_files/shared/build.sh` the rest. Before the first of them, `prepare.sh` removes the base image's
`ublue-os-*` packages that `system_files` replaces, and swaps Fedora's logo and release packages for
Fedora's generic ones, as its trademark guidelines ask of a remix: `fedora-logos` for `generic-logos`,
and the `fedora-release` packages for `generic-release` in the package database only, their files
staying in place.
1. `00-image-info.sh` - Sets image metadata and os-release info, and takes Fedora's name out of the other release files (`/usr/lib/fedora-release`, the CPE, the software identification tags)
2. `03-install-kernel-akmods.sh` - Installs the kernel and the kmods beside it: v4l2loopback, xpadneo and xone from Universal Blue's `akmods` image, evdi built here, ZFS on stable, and on the NVIDIA images the driver of `akmods-nvidia-open` or `akmods-nvidia-lts` (see Drivers)
3. `04-packages.sh` - Installs Fedora and COPR packages
4. `05-override-install.sh` - Overrides base image packages. Among them the multimedia stack: the base image installs negativo17's FFmpeg, built with `--enable-nonfree` and labelled by FFmpeg itself "nonfree and unredistributable", and its `pipewire-libs-extra`, whose LC3plus plugin links `liblc3plus`, which has no licence to redistribute. Both are replaced with Fedora's `ffmpeg-free` and `fdk-aac-free`, RPM Fusion's `libavcodec-freeworld`, and RPM Fusion's aptX plugin. negativo17's repository is `fedora-multimedia` in the base image and outranks Fedora's, so `17-cleanup.sh` turns it off; do not install FFmpeg, codec or PipeWire packages from it. `20-tests.sh` fails on any FFmpeg library that reports itself unredistributable. The script also deletes `/usr/share/doc`, after moving the licence, copyright and NOTICE files that some 120 packages keep only there into `/usr/share/licenses`, and installs Starship from a pinned release checked against its published checksum (the pin is bumped by Renovate)
5. `06-branding.sh` - Puts the Amethystora artwork (generated by `node branding/generate.mjs`) where the boot menu, the boot splash and GDM look for it: the GRUB font, the Plymouth theme, and Fedora's own logo files, which GDM, the Settings About page and the fallback splash look up by fixed path
6. `build_files/shared/build-gnome-extensions.sh` - Sets up the GNOME Shell extensions written for this image: their licences, schemas and entries in `enabled-extensions` (nothing is built from a git submodule any more)
6a. `09-fonts.sh` - The themes' interface fonts Fedora does not package, from Google Fonts' repository at one pinned commit, each file checked by SHA-256, into `/usr/share/fonts/amethystora` with their OFL texts in `/usr/share/licenses/google-fonts`, and Quicksand from `branding/fonts`; it fails when a theme's `font.theme` names a family that is not installed. The packaged ones are in `04-packages.sh`
7. `10-icons.sh` - Builds the icon caches of the Amethystora icon packs, which come with `system_files` (see The icon packs); nothing is downloaded. candy-icons, the pack the desktop shipped with until 2026-10-23, is gone, with its keep list, its aliases and the Security shield and Notes icon drawn from it (the Glass pack draws those two now, into `hicolor`). Nobody else's logo is drawn: an app of another maker keeps the icon it ships. `user-setup.hooks.d/16-retire-jetbrains-icons.sh` takes back the links an earlier image made to the IDE drawings
8. `11-gtk-theme.sh` - Installs the Amethystora GTK theme: EliverLara's Sweet, with every cool colour in it rotated onto the amethyst palette by `build_files/shared/recolor.py` (its stylesheets and drawings) and ImageMagick (the bitmaps named in the script, where the build fails on any still drawn in a cool colour: `recolor.py --check-bitmaps`). It ships under Amethystora's own name because the palette is no longer Sweet's. Only the `amethystora` and `amethystora-light` themes name it in their `gtk.theme`, for GTK 3; the rest stay on adw-gtk3. libadwaita ignores `gtk-theme` and a Flatpak cannot see `/usr/share/themes` at all, so the theme reaches them through `theme-set.hooks.d/20-gtk-apps.sh`: it mirrors the active theme into `~/.themes`, which `/etc/flatpak/overrides/global` grants every sandbox read-only, and writes the `~/.config/gtk-4.0/gtk.css` with the palette layer (see Colours in applications)
9. `12-login-screen.sh` - Patches the login screen background into GNOME Shell's own stylesheet. GNOME reads it from the `#lockDialogGroup` rule inside `gnome-shell-theme.gresource` and from nowhere else, so the bundle is unpacked, the rule appended and the bundle rebuilt. The picture is the crown the boot splash ends on, as sharp as the splash draws it and without the gem, drawn by `branding/generate.mjs`
10. `12-tour.sh` - Swaps the picture on GNOME Tour's first page, which Fedora builds with Fedora's logo on it, for Amethystora's, in the resource bundle the Tour reads its pictures from (`/usr/share/gnome-tour/resources.gresource`), unpacked and rebuilt as `12-login-screen.sh` does. The picture is the boot splash at the top of its glow, drawn by `branding/generate.mjs` into `/usr/share/amethystora/tour/welcome.svg`
11. `13-apps.sh` - Installs the Amethystora apps (the Manual, `Super+F1`; Security; Updates; Notes; Logs; Backups; Control; Help): the one program the Containerfile's `apps` stage built from `apps/` (`build_files/shared/build-apps.sh`, see The Amethystora apps), hard-linked to each app's own path in `/usr/lib/amethystora-<app>`, Quicksand for the wordmark from `branding/fonts`, which the `ctx` stage copies in, and the licences of the crates built into it. It also writes the apps' launchers in the languages the apps are translated into (`translate-desktop.py`). Each app's translations stay in `system_files/shared/usr/lib/amethystora-<app>/resources/app/locale`
12. `14-security.sh` - What Amethystora Security reports on, beside its window: derives network protection's Suricata configuration (`/usr/share/amethystora/ips/suricata.yaml`) from the one the suricata package ships, with `build_files/shared/ips-config.py`, so that it fits Suricata 7 and 8 alike, and compiles `po/*.po`, the report's and the app permissions' translations, into `/usr/share/locale/<language>/LC_MESSAGES/amethystora.mo` with `msgfmt --check`
13. `07-debrand.sh` - Renames every remaining Universal Blue file, command, service and reference to Amethystora (logic in `build_files/shared/debrand.py`; `20-tests.sh` fails the build if any is left). Licence files, copyright lines and the comments that credit an author keep upstream's names
14. `08-hardening.sh` - Signature-verified updates, firewall default zone, account lockout, Parental Controls' allowed hours at the text consoles and over SSH (see Logins), kernel lockdown, and the sudo PATH that `ublue-os/main` leaves open. The polkit and udev fixes are made in the files themselves (`org.amethystora.privileged.user.setup.policy`; `50-zsa.rules`, `60-arduino-mbed.rules`, `88-neutron_hifi_dac.rules` and `92-viia.rules`, which give their devices to the user at the seat with `uaccess` instead of to every account: a keyboard's hidraw device carries every key typed on it, and `20-tests.sh` fails on a rule the image adds that gives a hidraw device or a vendor's devices to everyone). Lockdown is skipped on the NVIDIA images, whose driver is an akmods build signed with the machine owner key: forcing lockdown on a machine with Secure Boot off would leave it without a graphics driver. With Secure Boot on, Fedora's kernel locks itself down whatever the arguments say (`20-tests.sh` checks its config), so the argument only matters without; `ame security lockdown` adds or takes it off per machine. It takes setuid root off what a desktop never runs (`chfn`, `chage`, `gpasswd`, `newgrp`, `ksu`, `mount.davfs`), masks systemd's SSH-over-vsock generator in `/etc` (`ame security ssh-settings off` unmasks it) and sets BlueZ's privacy and 30-second visibility in its own `main.conf`, which reads no drop-ins (`ame security bluetooth-privacy`). The settings that are plain files live in `system_files/shared` (`usr/lib/sysctl.d`, `usr/lib/modprobe.d`, `usr/lib/bootc/kargs.d`, `etc/ssh/sshd_config.d`, `etc/security/faillock.conf`, `etc/flatpak/overrides/global`, `etc/audit/rules.d`)
15. `17-cleanup.sh` - Cleanup operations, and the systemd units the image enables, the per-user ones with `systemctl --global` (`amethystora-pkg-upgrade.timer`, `amethystora-update-alert.service`, `amethystora-dns-portal.service`). It also makes the `amepkg` link, since the build's rsync skips links in `system_files`. Two things here are deliberately *disabled*: `input-remapper.service`, which runs as root and reads every input device, and `usbguard.service`, which would block the keyboard on a machine where nobody had allowed it yet. Both are turned on per machine by an `ame security` command (`usb`, `input-remapper`). So are `waydroid-container.service` and `scx_loader.service` (`ame apps android`, `ame system scheduler`). `usbguard-dbus.service` is *masked*, not disabled, since D-Bus activation would start it and GNOME's USB protection plugin would then allow every new device while the screen is unlocked: only `ame security usb locked` unmasks it. It enables `fwupd-refresh.timer`, which Fedora leaves to GNOME Software, the pools of disks' `amethystora-raid-check.timer`, `amethystora-raid-scrub.timer`, `amethystora-raid-boot.service` and `amethystora-raid-mirror.service`, and `clamd@scan.socket` with `amethystora-clamd-idle.timer` in place of clamd itself
16. `18-workarounds.sh` - Temporary fixes/workarounds
17. `19-initramfs.sh` - Regenerates initramfs, adding dracut's `tpm2-tss` module where dracut has it so that `ame security disk-unlock` can hand the disk key to the TPM, and without what changes with every build, so that two builds of the same packages make the same initramfs and an update between them can finish with a soft reboot (see The Updates app)
18. `build_files/shared/build-dx.sh` - dx only: copies `system_files/dx`, runs `build_files/dx/00-dx.sh` and then `01-tests-dx.sh`
18a. `build_files/shared/open-anything-types.py` - Run by `build.sh` after dx, once every package and launcher is in place: writes the kinds of file nothing in the image opens into `zz-amethystora-open-anything.desktop` and has the launchers' index made again (see Round 7's other groups)
18b. `build_files/shared/build-agent-index.sh` - The AI agent's index (`amethystora-agent-tools index --write`), once every name is final and every unit enabled or not, of the image as it then is, dx's files included (see Agentic features)
19. `build_files/shared/source-manifest.sh` - Writes `/usr/share/licenses/amethystora/SOURCES`: every package with its licence, source package and where that source is, which the GPL asks for and the written offer in `NOTICE` points to. Fedora keeps its own in Koji. RPM Fusion, negativo17 and Copr publish only their current builds, so the script fetches the source packages of theirs that are under a licence asking for the source into `/usr/src/amethystora`, and the build fails when one cannot be fetched; a Copr project the image starts installing from goes in its `COPR_PROJECTS`. NVIDIA's own repository, which the NVIDIA images take the container toolkit from, publishes no source packages at all, so its packages are listed with the address of their source instead (`NO_SOURCE_PACKAGES`); a GPL package from any other maker the script does not know still fails the build. `20-tests.sh` fails if a package is missing from the list or a source package from the image
20. `20-tests.sh` - Runs last, after the repositories are validated and the build cleaned up; fails the build on anything missing or left behind, including a GNOME Shell extension shipped without its licence

The sealed images then run `build_files/sealed/` from `Containerfile.sealed`, on the finished image (see Sealed images).

## Development Guidelines

### Making Changes
1. **ALWAYS validate first:** `just check && pre-commit run --all-files`
2. **Make minimal modifications** - prefer configuration over code changes
3. **Test formatting:** `just fix` to auto-format
4. **Avoid full container builds** unless specifically testing container changes, or asked to run the tests
   (`tests/vm`, the `run-tests` skill)
5. **Focus on system_files/ changes** for most user-facing modifications

### File Editing Best Practices
- **JSON files**: Validate syntax with `pre-commit run check-json`
- **YAML files**: Validate syntax with `pre-commit run check-yaml`
- **Justfile**: Always run `just check` after modifications
- **Shell scripts**: Follow existing patterns in build_files/

### Common Modification Patterns
- **Adding packages**: Edit `build_files/base/04-packages.sh`, add to appropriate array
- **System configuration**: Modify files in `system_files/shared/`
- **Build logic**: Edit scripts in `build_files/base/` or `build_files/dx/`
- **CI/CD**: Modify workflows in `.github/workflows/`

## Trust These Instructions

**The information in this document has been validated against the current repository state.** Only search for additional information if:
- Instructions are incomplete for your specific task
- You encounter errors not covered in the workarounds section
- Repository structure has changed significantly

This repository is complex but well-structured. Following these instructions will significantly reduce build failures and exploration time.

## Dependency Updates (Renovate)

Renovate is configured in `.github/renovate.json5` and tracks its work on its
Dependency Dashboard issue.

Never hand-edit a `renovate/*` branch and never merge `main` into one. Renovate
will not update or replace a branch it no longer owns; the PR moves to "PR
Edited (Blocked)" on the dashboard and that dependency silently stops updating.
Nothing about the PR looks wrong from the outside, so this is only visible on
the dashboard. `rebaseWhen` is set to `behind-base-branch` so Renovate keeps its
own branches current and there is no reason to touch them by hand.

Closing a Renovate PR is not a fix either. It moves the entry to "PR Closed
(Blocked)" and Renovate stops recreating it. Only close one once the dependency
has actually been updated by other means, and say so in the closing comment.

Renovate PRs are authored by `app/renovate`.

## Other Rules that are Important to the Maintainers

- Ensure that [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/#specification) are used and enforced for every commit and pull request title. A `feat` subject is shown to users as a line of What's new (see The manual): write it in plain words, or it is left out.
- Always be surgical with the least amount of code, the project strives to be easy to maintain.
- This project is published at arsslensoft/amethystora
- The desktop layer is maintained here, in `system_files/shared`; it is not pulled from upstream

## Attribution Requirements

AI agents must disclose what tool and model they are using in the "Assisted-by" commit footer:

```text
Assisted-by: [Model Name] via [Tool Name]
```

Example:

```text
Assisted-by: Claude 3.5 Sonnet via GitHub Copilot
```
