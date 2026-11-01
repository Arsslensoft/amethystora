#!/usr/bin/bash
# live.sh COMMAND [ARGS]: fixing live in a kept VM of the lab (.claude/skills/run-tests/live-fix.md). The VM is
# VMLAB_LIVE (default live), on a lab disk; its /usr is writable until it restarts (bootc usr-overlay), and files
# pushed since the last reset are put back after a restart. live.mjs does the work in the lab, from /vm/live.
#
#   live.sh start DISK [--fresh]    start the VM on an overlay of DISK (of NAME-ready when it exists), wait for the
#                                   boot (and the session), unlock /usr
#   live.sh ready                   sign the test account in (10-account.sh), wait for flatpak-preinstall, power off:
#                                   kept as the stopped VM NAME-ready, which every reset starts from
#   live.sh reset                   NAME started fresh from NAME-ready (or the disk), pushed files forgotten
#   live.sh save | restore          the running VM kept with its memory, and brought back running, without a boot
#   live.sh push FILE... | --since REV
#                                   files of the checkout (system_files, tests/vm/suites) into the guest with LF
#                                   endings, removed when the checkout has none, then what reads them reloaded
#   live.sh push-diff [REV]         push --since REV (default HEAD: what is not committed yet)
#   live.sh affected [REV]          the suites a change since REV touches (their "# vmlab-paths:" lines)
#   live.sh round [REV]             push-diff, then the affected suites: one fix round
#   live.sh delta REV               for what a push cannot reach: an image FROM the VM's own (DELTA_BASE, default
#                                   localhost/amethystora-live:DISK) with the working tree's system_files and only the
#                                   build steps they feed since REV (delta.sh; DELTA_APPS=an apps stage for apps/),
#                                   then the VM switched to it (bootc switch, a soft reboot when it allows one)
#   live.sh relogin                 the test account's session started again
#   live.sh suite(s) NAME...        the checkout's suites put in and those run in turn (11, 11-dock); logs and
#                                   screenshots in the lab
#   live.sh exec [--user U] [--session] COMMAND
#   live.sh shot [FILE.png]         the screen, copied out to FILE
#   live.sh app                     the apps' program built from the checkout (amethystora-rust) and put in
#   live.sh stop | delete
set -euo pipefail

export MSYS_NO_PATHCONV=1
cd "$(dirname "$0")/../.."
VM="${VMLAB_LIVE:-live}"
LAB="/vm/live/${VM}"
if ! command -v docker >/dev/null && command -v cygpath >/dev/null; then
    PATH="$(cygpath -u "${LOCALAPPDATA}")/Programs/DockerDesktop/resources/bin:${PATH}"
fi
lab() { docker exec -e VMLAB_MEM="${VMLAB_MEM:-}" amevm node /vm/live/live.mjs "${VM}" "$@"; }

# The suites a change since REV touches: a path matching one of a suite's "# vmlab-paths:" globs (@sf/ is
# system_files/shared/, @ui/ Amethystora UI's folder, * matches / too), the suite itself, lib.sh (every suite), or a
# helper beside the suites that the suite names. What a push cannot reach is said on stderr.
affected() {
    local changed suite name globs g f hit ui=system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@iarsslen.github.io/
    changed="$({ git -c core.safecrlf=false diff --name-only "$1"; git ls-files --others --exclude-standard; } | sort -u)"
    grep -E '^(apps|build_files|po|packaging|branding|iso)/|^Containerfile' <<<"${changed}" |
        sed 's/^/live.sh: not pushed, needs an image build: /' >&2 || true
    local line
    declare -A paths=()
    while IFS= read -r line; do paths["${line%%:*}"]="${line#*:# vmlab-paths: }"; done \
        < <(grep -H '^# vmlab-paths: ' tests/vm/suites/[0-9][0-9]-*.sh | tr -d '\r')
    set -f
    for suite in $(printf '%s\n' "${!paths[@]}" | sort); do
        name="${suite##*/}"
        name="${name%.sh}"
        globs="${paths[${suite}]}"
        globs="${globs//@sf\//system_files/shared/}"
        globs="${globs//@ui\//${ui}}"
        hit=
        while IFS= read -r f; do
            [[ -n "${f}" ]] || continue
            if [[ "${f}" == "${suite}" || "${f}" == tests/vm/suites/lib.sh ]]; then hit=1; break; fi
            if [[ "${f}" == tests/vm/suites/* && "${f}" != *.sh ]] && grep -qF "${f##*/}" "${suite}"; then hit=1; break; fi
            for g in ${globs}; do
                if [[ "${f}" == ${g} ]]; then hit=1; break 2; fi
            done
        done <<<"${changed}"
        [[ -z "${hit}" ]] || echo "${name}"
    done
    set +f
}
if [[ "${1:-}" == affected ]]; then
    affected "${2:-HEAD}"
    exit
fi
docker exec amevm mkdir -p "${LAB}"
# The checkout's lab code beside live.mjs, with LF: a run's copy in /opt can be older
for f in live.mjs lab.mjs agent.py make-disk.sh; do tr -d '\r' <"tests/vm/${f}" | docker exec -i amevm sh -c "cat >/vm/live/${f}"; done
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

case "${1:-}" in
push)
    shift
    if [[ "${1:-}" == --since ]]; then
        { git diff --name-only --no-renames "${2:?push --since REV}" -- system_files tests/vm/suites
            git ls-files --others --exclude-standard -- system_files tests/vm/suites; } | sort -u >"${tmp}/list"
    else
        printf '%s\n' "${@:?push FILE...}" >"${tmp}/list"
    fi
    # Modes from git (the checkout on Windows has none), else executable when Git Bash says so (a #! line)
    declare -A tracked=()
    while IFS=$'\t' read -r meta file; do tracked["${file}"]="${meta%% *}"; done < <(git ls-files -s -- system_files tests/vm/suites)
    while IFS= read -r f; do
        if [[ ! -e "${f}" ]]; then
            echo "- ${f}"
        elif [[ "${tracked[${f}]:-}" == 100755 || ( -z "${tracked[${f}]:-}" && -x "${f}" ) ]]; then
            echo "0755 ${f}" && echo "${f}" >>"${tmp}/files"
        else
            echo "0644 ${f}" && echo "${f}" >>"${tmp}/files"
        fi
    done <"${tmp}/list" >"${tmp}/push.new"
    [[ ! -e "${tmp}/files" ]] || tar -c -T "${tmp}/files" | docker exec -i amevm sh -c "mkdir -p ${LAB}/files && tar -x -C ${LAB}/files"
    docker exec -i amevm sh -c "cat >${LAB}/push.new" <"${tmp}/push.new"
    lab push
    ;;
push-diff)
    bash "$0" push --since "${2:-HEAD}"; exit $?
    ;;
suite | suites)
    shift
    tar -c -C tests/vm/suites . | docker exec -i amevm sh -c "rm -rf ${LAB}/suites && mkdir -p ${LAB}/suites && tar -x -C ${LAB}/suites"
    failed=0
    for name in "${@:?suite NAME...}"; do lab suite "${name}" || failed=1; done
    exit "${failed}"
    ;;
delta)
    # REV: the commit the VM's image was built from. The working tree as one commit, through an index of its own so
    # that the checkout's is untouched, sent as a bundle: the lab checks it out with LF, as a build sees it
    rev="${2:?delta REV: the commit the image of the VM was built from}"
    base="${DELTA_BASE:-localhost/amethystora-live:$(docker exec amevm cat "${LAB}/disk")}"
    cp "$(git rev-parse --git-dir)/index" "${tmp}/index"
    GIT_INDEX_FILE="${tmp}/index" git add -A
    commit="$(git commit-tree "$(GIT_INDEX_FILE="${tmp}/index" git write-tree)" -p HEAD -m "live.sh delta")"
    git update-ref refs/live/delta "${commit}"
    git bundle create "${tmp}/tree.bundle" refs/live/delta 2>/dev/null
    git update-ref -d refs/live/delta
    git -c core.safecrlf=false diff --name-only "${rev}" "${commit}" >"${tmp}/changed"
    docker exec -i amevm sh -c "cat >${LAB}/tree.bundle" <"${tmp}/tree.bundle"
    tr -d '\r' <tests/vm/delta.sh | docker exec -i amevm sh -c "cat >/vm/live/delta.sh"
    docker exec amevm sh -c "rm -rf ${LAB}/ctx && git init -q ${LAB}/ctx && git -C ${LAB}/ctx fetch -q ${LAB}/tree.bundle refs/live/delta &&
        git -C ${LAB}/ctx checkout -q FETCH_HEAD"
    docker exec -i amevm sh -c "cat >${LAB}/ctx/.live-changed" <"${tmp}/changed"
    docker exec amevm bash /vm/live/delta.sh "${base}" "${VM}" "${LAB}/ctx" ${DELTA_APPS:-}
    lab switch "10.0.2.2:5000/amethystora-delta:${VM}"
    ;;
round)
    # One fix round: what changed since REV pushed in, then the suites it touches
    bash "$0" push --since "${2:-HEAD}"
    mapfile -t names < <(affected "${2:-HEAD}")
    [[ ${#names[@]} -gt 0 ]] || { echo "live.sh: no suite names a changed path" >&2; exit 0; }
    bash "$0" suites "${names[@]}"; exit $?
    ;;
ready)
    tar -c -C tests/vm/suites . | docker exec -i amevm sh -c "rm -rf ${LAB}/suites && mkdir -p ${LAB}/suites && tar -x -C ${LAB}/suites"
    lab ready
    ;;
shot)
    lab shot
    [[ -z "${2:-}" ]] || docker cp "amevm:${LAB}/shot.png" "$(cygpath -m "$2" 2>/dev/null || echo "$2")"
    ;;
app)
    # In the Rust image beside the lab, on 4 processors, with its build cache in the volume amethystora-target
    src="$(cygpath -m "${PWD}" 2>/dev/null || echo "${PWD}")/apps"
    docker run --rm --cpus 4 -v "${src}:/src" -v amethystora-cargo:/root/.cargo/registry -v amethystora-target:/target \
        -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS=4 -e CARGO_TERM_COLOR=never -w /src --entrypoint cargo amethystora-rust \
        build --release --locked -p amethystora-apps
    docker run --rm -v amethystora-target:/target --entrypoint cat amethystora-rust /target/release/amethystora-apps |
        docker exec -i amevm sh -c "cat >${LAB}/amethystora-apps"
    lab app
    ;;
*)
    lab "$@"
    ;;
esac
