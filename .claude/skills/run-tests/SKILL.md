---
name: run-tests
description: Build Amethystora from the last commit, boot it in the VM lab, run the test suites, and fix what fails until it passes. Use only when the user explicitly asks to run the tests ("run the tests", "build and test", "test it in the VM").
---

# Running the tests and fixing what fails

A run builds the whole image from the last commit (`just build`), installs it onto a disk, boots it with
Secure Boot and a TPM in the VM lab (`tests/vm`), and runs the suites in `tests/vm/suites` in it. It takes
an hour or more, so start one only when the user asks. The maintainer's rules for it (2026-10-11):

- A run builds **committed HEAD**, never the working tree.
- **As few build and test round trips as possible** (the maintainer's rule of 2026-10-29). While iterating, run
  only **the suites of the current work** (its own suites and those of the areas it changed: `run.sh test --keep
  --suites NN,NN`), never all of them. Fix every failure **live in the kept VM**, without rebuilding it, until
  the issue is really gone (read the log and the screenshot, not only the count); rerun the suite once more to
  be sure it is stable. Then carry the fixes back here, commit them, and make **one final build** with all of
  them. On that build, run **the full suite only when the changes are risky** (packages, the build, boot,
  security, PAM, the kernel, the image's units, or a shared library many suites use); otherwise a final test of
  the current work's suites only. Every bug a run finds is fixed, not only reported. How:
  [live-fix.md](live-fix.md).
- When a local build cannot run, a **GitHub** build on a test branch takes its place.

How to read a failure, try a fix without a full rebuild, and tell the lab's problems from the image's:
[diagnosing.md](diagnosing.md). How the lab works: `tests/vm/README.md`.

## 1. Before the run

1. `git status`: commit your own finished work first, staging your files by name. Other sessions work in
   this tree; their uncommitted files stay out of the run and out of your commits.
2. On Windows, Docker Desktop's CLI is not on PATH in the Bash tool:
   `export PATH="$(cygpath -u "$LOCALAPPDATA")/Programs/DockerDesktop/resources/bin:$PATH"`, and
   `MSYS_NO_PATHCONV=1` before any `docker exec` that names a path inside the lab.
3. Nothing else to prepare: `run.sh` starts the lab container (`amevm`) when it is stopped, loads KVM when
   Docker lost it, and brings the lab's code up to date from the commit. Only a change to
   `tests/vm/Containerfile` needs `bash tests/vm/setup.sh` first.

## 2. Run

```bash
bash tests/vm/run.sh build >SCRATCHPAD/run.log 2>&1 && bash tests/vm/run.sh disk >>SCRATCHPAD/run.log 2>&1
bash tests/vm/run.sh test --keep --suites NN,NN >>SCRATCHPAD/run.log 2>&1   # the current work's suites, VM kept
```

The full suite, only when the changes are risky (the rule above), runs with `--parallel N`: a ready state is made
once (installed, signed in, the first boot's Flatpaks in), and the suites are spread over N VMs started from it, by
their minutes in `tests/vm/shards.json` (`tests/vm/README.md`, Several VMs at once). Without it, the suites run one
after another on one VM: the fallback when the lab has no memory to spare for more. `summary.md` names the VM each
suite ran on; each VM's log lines start with its name.

Start it with `run_in_background: true` and the longest timeout (2 hours); you are told when it ends. Do not
pipe it into `tail`: that hides its exit status. The run carries on inside the lab even if the command is
cut off, and writes as it goes:

```bash
docker exec amevm cat /vm/runs/last/run.log       # one line per stage and per suite
docker exec amevm cat /vm/runs/last/summary.md    # at the end: stages, suites, every "not ok"
```

To follow it, wait in the background for a line, never with a chain of `sleep`s:

```bash
until docker exec amevm grep -q "== build: \(passed\|FAILED\)" /vm/runs/last/run.log; do sleep 30; done
```

`run.sh build`, `run.sh disk` and `run.sh test` run one stage from what the last run left; `test --keep`
leaves VM `test` running afterwards for the `amethystora-vm` MCP tools. Careful: after a failed build,
`run.sh test` boots the **previous** disk, not your change.

## 3. When something fails

Work through [diagnosing.md](diagnosing.md):

1. Find the first real error: the build's failing script and command, or the suite's `not ok` with its
   indented reasons, its log, its screenshot and the guest's journal.
2. Decide whether the lab or the image is wrong. A check that is itself wrong is fixed in the suite.
3. Fix it live in the kept VM ([live-fix.md](live-fix.md)): put the changed file into the running guest,
   reload what reads it, rerun that suite or step, and repeat until it passes there. Only what cannot be
   changed in a running machine (the kernel, the initramfs, a package, `20-tests.sh` itself) waits for the
   final run; check such a fix in a container of the image first.
4. Fix it at its cause, in the fewest lines. Never weaken, skip or delete a check to get a pass.
5. Commit it: `fix(scope): what was wrong`, a body saying what the run found, and the footer
   `Assisted-by: <model> via Claude Code` (no Co-Authored-By line). One commit per cause.
6. Add the check that would have caught it, if nothing did: `20-tests.sh` for what is true of the image
   as built, `tests/vm/suites` for what has to happen on a running machine.
7. When every failure is fixed, validated live and stable on a second pass, one final build of the commits,
   then the full suite if the changes are risky (see the rule above), else the current work's suites only.

**Code that is not yours.** Committed code from another session is part of HEAD, and fixing it is part of
the run. But a file with uncommitted changes in `git status` is being edited right now: report the failure
instead of editing it.

**What needs the user.** A design choice, a secret, a decision between two behaviours, a change upstream,
or anything the fix would take away from a user: stop on it, report it with the evidence, and go on with
the rest.

**When to stop.** Three final runs, or three attempts at the same build error, then report.
If the build cannot run here at all, go to section 5.

## 4. Report

- The summary table: each stage with its time, each suite with its counts.
- The commits the run made, one line each, and what each fixed.
- Measurements the suites printed (lines starting with `#`, such as Falco's memory).
- What is left, why, and what it needs from the user.
- The run's folder in the lab (`/vm/runs/<when>`), for the screenshots and logs.

When the final run of the whole suite is green, go on with the `queue-latest` skill: GitHub's Latest build of
the validated commit, queued and followed.

## 5. GitHub instead of a local build

Only when the build cannot run here (out of disk: `docker system df`; out of memory; a registry that does not
answer). Never push an unvalidated commit to `main` (a validated one goes there through `queue-latest`); `gh` is
logged in as the maintainer, and git pushes over SSH.

```bash
sha=$(git rev-parse --short HEAD)
git push origin "HEAD:refs/heads/test/${sha}"
gh workflow run "Latest Images" --repo arsslensoft/amethystora --ref "test/${sha}"
gh run list --repo arsslensoft/amethystora --workflow "Latest Images" --branch "test/${sha}" --limit 1
gh run watch RUN_ID --repo arsslensoft/amethystora --exit-status
gh run view RUN_ID --repo arsslensoft/amethystora --log-failed | tail -n 200
git push origin --delete "test/${sha}"     # when done
```

A branch build publishes nothing (`PUBLISH` in `reusable-build.yml`): it checks the build and `20-tests.sh`
only. The VM suites then wait for a local build, or for the image `main` publishes after the user pushes
(tag it `localhost/amethystora:latest` in the lab, then `run.sh disk` and `run.sh test`).

## Keeping the suites growing

A new feature gets its check in `tests/vm/suites` with it, and a bug a run found gets the check that would have
caught it. A suite is `NN-name.sh`, TAP lines from `lib.sh`'s helpers, ending with `finish`; exit 100 reboots
and runs it again with `SUITE_PHASE` one higher (`tests/vm/README.md`). A change to the lab or a suite is
tested by the next run once committed.
