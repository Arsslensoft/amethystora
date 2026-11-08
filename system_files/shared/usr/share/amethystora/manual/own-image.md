# An image of your own

For what has to be in the system itself, such as a driver, a system service or a tool every account on the
machine needs, build an image of your own on top of Amethystora rather than
[layering](software.md#layering-the-last-resort) packages.

### The image template

The [image template](https://github.com/arsslensoft/amethystora-image-template) is a repository to start
from: a `Containerfile` that builds `FROM` Amethystora, a script where your packages and files go, and a
GitHub workflow that builds the image whenever you push and once a day, signs it with a key of your own and
publishes it to your account's registry. Renovate moves it onto each new Amethystora release.

Its build ends with `/usr/libexec/amethystora-image-check`, which fails it if a step of yours undid something
Amethystora relies on: the checking of updates, the `ame` commands, or the absence of Fedora's logo and
release packages.

### Your apps, look and policy, without a script

- **A [setup file](setup.md)** (`ame setup save setup.json` writes this machine's) becomes the image's
  defaults.
- **A `branding/` folder** gives it your name, wallpaper, logo, login screen, desktop style, icons and fonts
  (About says "*your name*, built on Amethystora").
- **A `policy/` folder** sets how far the AI agent may go, Security's switches, the stricter hardening,
  GNOME's settings by default or locked for everyone (the screen lock, a notice on the login screen, the
  camera, printing, keyboard layouts and more), when machines update, and the company's proxy, DNS servers,
  VPN connections and Flatpak remotes. Never with a password or a key in them: each person gives those as
  they connect.
- **A `packages` file** adds Fedora's packages the system itself needs.

The [image editor](https://amethystora.org/build) on Amethystora's website writes these files for you. A
**Build ISO** workflow in the repository makes your image's two ISOs, with Amethystora's own installer files:
the one that starts from a USB stick to try it, and the installer.

## Switching to it

Your image is signed with your key, not Amethystora's, so tell this machine to take it only with your
signature before you switch. `cosign.pub`, from your repository, is the public half of the key:

```bash
ame system trust-image ghcr.io/<you>/<your-image> cosign.pub
sudo bootc switch --enforce-container-sigpolicy ghcr.io/<you>/<your-image>:stable
```

[Control](control.md)'s **System** page does the first line for you: type the repository (in lower case,
with its registry, without a tag; Amethystora's own are refused), choose the `cosign.pub`, and it opens in a
terminal, where it asks for your password. Once it is done, the card shows the `bootc switch` line to run
next.

### Why trust comes first

Without the first line, this machine would take your image whether it is signed or not. Your image says the
same of itself as it is built, so its updates are checked against your key once the machine runs it. Then
hand the signature policy back to the image, so that later changes to it arrive with updates rather than
staying as they were on the day you switched:

```bash
sudo cp /usr/etc/containers/policy.json /etc/containers/policy.json
```

`ame system rebase` offers only Amethystora's own images, and takes signed ones only. A
[sealed machine](streams.md#sealed-images) cannot switch to an image of your own.
