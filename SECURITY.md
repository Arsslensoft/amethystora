# Security policy

## Reporting a vulnerability

Report a security problem privately, not in a public issue: use
[Report a vulnerability](https://github.com/arsslensoft/amethystora/security/advisories/new) on this
repository's Security tab. The report stays between you and the maintainers until a fixed image is
out, and the advisory is published with it. Please keep the details to yourself until then.

Say as much as you can of:

- the image you found it on (`rpm-ostree status` names it exactly),
- what an attacker gains, and from where: another user, an app, the network, physical access,
- the steps that show it.

## Scope

This repository builds the Amethystora images. A report belongs here when the problem is in something
it makes or sets:

- the image's own programs, scripts and configuration (`build_files/`, `system_files/`),
- how the images and ISOs are built, signed and published (`.github/workflows/`),
- the published images and ISOs themselves.

A problem in a package that Fedora or Universal Blue ships unchanged belongs to that project, so
report it there. When it is not clear which it is, report it here.

## Supported versions

Fixes go into new images, and a system gets them with its next update (`ame update`). An older image
is not patched in place.
