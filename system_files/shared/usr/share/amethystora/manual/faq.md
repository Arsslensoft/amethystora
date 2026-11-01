# Questions and answers

## Why can I not use `dnf install`?

Because the system is an image, and the image is the same on every machine. That is what makes
updates safe and rollbacks instant. Apps come from Flathub, tools from Homebrew, and anything else
from a container; [Installing software](software.md) shows which is which. In a Fedora container
(`amepkg containers new --template fedora`), `dnf install` works exactly as you know it. On the
system itself `dnf` has no package database it can write to; `rpm-ostree install` can add a package to
the image on this machine, but it is the [last resort](software.md#layering-the-last-resort): every
layered package makes every update slower.

## Can I use another browser?

The browser is Firefox, exactly as Fedora builds it: Amethystora adds no settings or extensions to
it. You can ask it, and the other browsers, to stop [sending data to their makers](security.md#browser-protection). Chrome, Brave and every other browser are in Bazaar. To make one the default, open
**Settings → Apps → Default Apps**; [web apps](web-apps.md#which-browser-runs-them) then open in it
too.

## Where is Brave?

Amethystora used to come with Brave. It is in Bazaar now, like the other browsers. What you had in
it is still in `~/.config/BraveSoftware`, and Firefox can bring it over: **Settings → General →
Import Browser Data**.

## Is this Fedora?

Underneath, yes: Amethystora is built on Fedora's atomic desktop and GNOME, and follows Fedora's
releases. What it adds is the desktop around it, the hardening, the tools, and an image that is
tested and signed before it reaches you.

## Can I play games?

Yes. `ame apps gaming` installs Steam with what its games need, and grants Steam the X11 it draws
through, which every other app is refused. Many Windows games run through Proton in Steam.
[Games and Android apps](games.md) says what it sets up, and how to add Heroic or Lutris.

## Can I run Android apps?

Yes, in Waydroid: `ame apps android` sets it up. [Games and Android apps](games.md#android-apps).

## Can I run Windows software?

Often. Double-click the program in Files: it is scanned, and the same app for Linux, its web version or an
app that does the same job is offered first; otherwise it runs with **Bottles**, from Flathub, which it sets up
for you. For the rest, a Windows virtual machine in the [developer image](developer.md) works for nearly
everything. [Windows programs](software.md#windows-programs) says what happens and what will not run.

## How do I run an AppImage?

Install **Gear Lever** from Bazaar. It puts AppImages in the app grid and keeps them updated. Until
then, double-clicking one in Files does nothing, on purpose: make it executable and run it from a
terminal if you trust it.

## How do I install a `.deb` or an `.rpm`?

Open it in Files. It goes into a container made for its kind of package, after a virus scan and a
look at where it comes from; the same app from Flathub, if there is one, is offered first.
[A package file](software.md#a-package-file).

## Where do my settings live?

In your home folder: `~/.config` and `~/.local` for apps and the desktop,
`~/.config/amethystora` for the theme and the rest of Amethystora's own settings, and
`~/.var/app` for each Flatpak. Back up your home folder and you have backed up everything that is
yours.

## Can I sign in with a work or school account?

Yes, if it belongs to an Active Directory domain. Join the machine to the domain in **Settings ›
System › Users › Add User › Enterprise Login**, or in a terminal:

```bash
sudo realm join example.com
```

The domain's accounts can then log in here beside yours, and each gets a home folder at its first
login. Fingerprint login, security keys and the lock after ten wrong passwords keep working. To let in
only some of them, run `sudo realm deny --all`, then `sudo realm permit name@example.com` for each.
`sudo realm leave` takes the machine out of the domain again.

A FreeIPA domain needs its client first: `sudo rpm-ostree install freeipa-client`, then restart
([Layering](software.md#layering-the-last-resort)). Its installer sets up logins its own way and turns
account lockout, fingerprint login and security keys off; turn them back on with
`sudo authselect enable-feature with-faillock`, and the same for `with-fingerprint` and `with-pam-u2f`.
A plain LDAP or Kerberos directory is set up by hand in `/etc/sssd/sssd.conf`, as its administrators
describe; SSSD's support for both is in the image. Then turn its logins on with
`sudo /usr/libexec/amethystora-domain-logins sssd`, which keeps those three.

## Why six workspaces, always?

So that a number always means the same place. With workspaces that come and go, `Super+3` depends on
what you had open an hour ago; with six fixed ones, the browser on 1 and the terminal on 2 are
always exactly where your fingers expect them.

## Why can I not change files in `/usr`?

`/usr` is the image, read-only on purpose, and replaced as a whole by every update. Settings that
belong to the whole machine go in `/etc`, where your changes are kept and merged with each update.
Settings for your account go in your home folder.

## Can I trust the image?

It is built in public on GitHub from the source in
[arsslensoft/amethystora](https://github.com/arsslensoft/amethystora), signed with a key whose public half
ships in every image, and only accepted by your machine if that signature checks out.
