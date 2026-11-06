#!/usr/bin/bash

set -eoux pipefail

echo "::group:: ===$(basename "$0")==="

# Install tooling, unless the packages layer did (layers/packages.sh), in which case build.sh takes it out
GLIB_DEVEL_ADDED=false
if ! rpm -q glib2-devel >/dev/null; then
    dnf5 -y install glib2-devel
    GLIB_DEVEL_ADDED=true
fi

# Each extension installed below is added to the enabled-extensions list in zz0-amethystora-modifications,
# so that installing one is enough to have it on. Adding one that is already in the list would enable
# it twice, so this only appends when it is absent - and fails if there is no list to append to,
# rather than leaving an extension built into the image but off.
BASE_OVERRIDE=/usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override

enable_extension() {
    local uuid="$1"
    grep -q "'${uuid}'" "${BASE_OVERRIDE}" ||
        sed -i "/^enabled-extensions/ s/\]/, '${uuid}'&/" "${BASE_OVERRIDE}"
    grep -q "'${uuid}'" "${BASE_OVERRIDE}"
}

# Install Extensions

# Amethystora Transitions: written for this image, so it is in system_files rather than a submodule,
# and under the image's own licence. Its schema is installed system-wide from there too, so that
# gsettings and ame desktop transition reach it; the compile at the end picks it up.
install -Dpm0644 /usr/share/licenses/amethystora/LICENSE /usr/share/licenses/amethystora-transitions@arsslensoft.github.io/LICENSE
enable_extension "amethystora-transitions@arsslensoft.github.io"

# Amethystora Clipboard: the clipboard history on Super+Shift+V. A copy of Clipboard Indicator v71 (MIT, by
# Yotam Bar-On and its contributors, https://github.com/Tudmotu/gnome-shell-extension-clipboard-indicator at
# c880c7fb88dc7232a61a5864d125989a4f375daa) that seals its history with /usr/libexec/amethystora-clipboard-seal
# (built in the apps stage) instead of writing it to ~/.cache in plain text. In system_files with upstream's
# LICENSE.rst; its schema is system-wide, and 14-security.sh compiles its translations (po/amethystora-clipboard).
enable_extension "amethystora-clipboard@arsslensoft.github.io"

# Amethystora Logo Menu: the gem at the left of the top bar and its menu of the system's tools. Written for this
# image, in system_files and under the image's licence, with no settings; its labels are in po/ (domain amethystora).
install -Dpm0644 /usr/share/licenses/amethystora/LICENSE /usr/share/licenses/amethystora-logo-menu@arsslensoft.github.io/LICENSE
enable_extension "amethystora-logo-menu@arsslensoft.github.io"

# Amethystora UI: the desktop's own modules, each written for this image in place of a third-party extension
# (plans/PLAN-amethystora-ui.md), in system_files and under the image's licence. A module's keys are in its own
# schema in the extension's schemas folder, installed system-wide too so that gsettings, ame and the overrides reach
# them; the loop below compiles them with the overrides.
UI_EXTENSION=/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io
install -Dpm0644 /usr/share/licenses/amethystora/LICENSE /usr/share/licenses/amethystora-ui@arsslensoft.github.io/LICENSE
if compgen -G "${UI_EXTENSION}/schemas/*.gschema.xml" >/dev/null; then
    install -Dpm0644 -t /usr/share/glib-2.0/schemas "${UI_EXTENSION}"/schemas/*.gschema.xml
fi
enable_extension "amethystora-ui@arsslensoft.github.io"

# The image's version is each extension's: Extension Manager's row shows a version only when "version" is there, and
# then "version-name", and GNOME's Extensions app shows the name too. Nothing updates these from extensions.gnome.org.
for metadata in /usr/share/gnome-shell/extensions/amethystora-*@arsslensoft.github.io/metadata.json; do
    stamped="$(jq --arg name "${VERSION:-00.00000000}" '. + {version: 1, "version-name": $name}' "${metadata}")"
    printf '%s\n' "${stamped}" >"${metadata}"
done

# GNOME Shell's getSettings() reads an extension's own compiled schemas before the system's, and the
# overrides in /usr/share/glib-2.0/schemas never reach those: without this, every extension above ran
# on its own defaults. So each one's schemas are compiled again with its sections of zz0- and
# zz1-amethystora-modifications, strictly, so that a key it no longer has fails the build rather than
# being dropped without a word.
for schemas in /usr/share/gnome-shell/extensions/*/schemas; do
    # Some schema files spread a tag over several lines
    ids="$(cat "${schemas}"/*.gschema.xml | tr -s '[:space:]' ' ' | grep -oE '<schema [^>]*id="[^"]+"' |
        sed -E 's/.*id="([^"]+)".*/\1/' | tr '\n' ' ')"
    awk -v ids="${ids}" 'BEGIN { n = split(ids, list, " "); for (i = 1; i <= n; i++) own["[" list[i] "]"] = 1 }
        /^\[/ { keep = ($0 in own) } keep' \
        /usr/share/glib-2.0/schemas/zz0-amethystora-modifications.gschema.override \
        /usr/share/glib-2.0/schemas/zz1-amethystora-modifications.gschema.override \
        >"${schemas}/zz-amethystora-modifications.gschema.override"
    glib-compile-schemas --strict "${schemas}"
    rm "${schemas}/zz-amethystora-modifications.gschema.override"
done

rm /usr/share/glib-2.0/schemas/gschemas.compiled
glib-compile-schemas /usr/share/glib-2.0/schemas

# Cleanup
if [[ "${GLIB_DEVEL_ADDED}" == true ]]; then
    dnf5 -y remove glib2-devel
fi
rm -rf /usr/share/gnome-shell/extensions/tmp

echo "::endgroup::"
