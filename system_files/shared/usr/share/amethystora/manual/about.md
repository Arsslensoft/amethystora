# About

Amethystora is created and maintained by Arsslen Idadi ([@arsslensoft](https://github.com/arsslensoft)).
It is free software under the Apache License 2.0, built in the open at
[github.com/arsslensoft/amethystora](https://github.com/arsslensoft/amethystora). Issues, ideas and pull
requests are welcome there.

Amethystora is based on [Universal Blue](https://universal-blue.org), copyright its contributors and licensed under the Apache License 2.0.

## This manual

The pages are Markdown files in `/usr/share/amethystora/manual`, one per page, and the hotkeys page
is `/usr/share/amethystora/keybindings.md`, the same file `ame desktop keybindings` shows. They come with
the image, so the manual always describes the system you are running, and it works without a network
connection.

```bash
amethystora-manual                 # open it where you left off
amethystora-manual updates         # open a page by its file name
amethystora-manual updates#rolling-back   # or at one of its sections
amethystora-manual --list          # the pages there are
```

Inside the manual: `Ctrl+K`, `Ctrl+F` or `/` searches for the sections that have every word you type,
`Up`, `Down` and `Enter` pick one and `Esc` goes back to the page; `Alt+Left` and `Alt+Right` go back
and forward, `Ctrl+=` and `Ctrl+-` change the text size, and `Ctrl+0` resets it. Every command has a
**Copy** button. A page shows in the session's language where it has been translated, and in English
otherwise.

French, Arabic, Spanish and German were first translated by machine, and no person has read them all yet: until a reader has approved the security report and the welcome page in a language, take a doubtful sentence there as a question and check it against the English. What the `ame` commands say and ask, and the text of a notification, show in your language where there is a translation of it and in English where there is not yet; what the security watcher found, the names of files and programs, and what other programs print stay as they were written. To read, correct or add a language, see [CONTRIBUTING.md](https://github.com/arsslensoft/amethystora/blob/main/CONTRIBUTING.md#translating).

## Standing on the shoulders of

Amethystora is built on [Fedora](https://fedoraproject.org) and [GNOME](https://www.gnome.org), and
on the work of many other projects: the Linux kernel and systemd, bootc and rpm-ostree, Flatpak and
Flathub, Homebrew, Distrobox and Podman, and every GNOME extension listed in
[Getting around](desktop.md#extensions). The icon packs are drawn for Amethystora, and the Amethystora GTK
theme is recoloured from EliverLara's [Sweet](https://github.com/EliverLara/Sweet). The themes' wallpapers are drawn for Amethystora.

Fedora and the Fedora logo are trademarks of Red Hat, Inc. Amethystora is not provided, supported or
endorsed by the Fedora Project or by Red Hat. Fedora itself is at
[fedoraproject.org](https://fedoraproject.org).

The licences of everything in the image are in `/usr/share/licenses`. A few parts are not free
software and come under their makers' own terms: device firmware, the DisplayLink driver for docks,
and the NVIDIA driver on the NVIDIA images. `/usr/share/licenses/amethystora/NOTICE` lists them.

## Source code

Every package in the image is listed in `/usr/share/licenses/amethystora/SOURCES`, with its licence,
the source package it was built from and where that source is. The source of a Fedora package is in
Fedora's build system at the address given there, or one command away:

```bash
dnf download --srpm <package>    # the source of the version Fedora has now
```

A few packages are built by others (RPM Fusion, negativo17, Fedora Copr), who publish only their
current build of each. Where such a package's licence asks for its source, the source package this
image was built from is in the image itself, in `/usr/src/amethystora`.

Everything else is built from [the Amethystora repository](https://github.com/arsslensoft/amethystora)
at the commit named as `BUILD_ID` in `/usr/lib/os-release`. For at least three years after an image
was last published, the complete source of anything in it whose licence asks for its source to be
available (the GNU GPL and LGPL, the MPL, the CDDL, the Fraunhofer AAC codec's licence) is also
given to anyone who asks through the [issue tracker](https://github.com/arsslensoft/amethystora/issues);
the written offer is in `/usr/share/licenses/amethystora/NOTICE`.
