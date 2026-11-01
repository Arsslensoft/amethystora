---
name: queue-latest
description: Queue GitHub's "Latest Images" build of a commit the VM lab has just validated. Use it as soon as a run-tests final run of the whole suite is green (every suite passed, or only the lab's known leftovers named in the report), and when the user asks to "queue the latest build" after a green run. Not for a partial or failing run.
---

# Queueing the Latest build of a validated commit

The maintainer's rule (2026-10-30): once a commit is fully built and validated in the VM lab, GitHub's
`Latest Images` workflow builds it from `main`, which publishes `:latest` (`PUBLISH` in `reusable-build.yml`).
`gh` is logged in as the maintainer (admin on `arsslensoft/amethystora`).

## 1. Only a validated commit

- The run's `summary.md` (`docker exec amevm cat /vm/runs/last/summary.md`) names the commit it built; call it
  `SHA`. Every stage passed and every suite is green, or each `not ok` left is a lab cause already named in the
  run's report (the multi-screen switch press, a lab-only SELinux denial on the live overlay). Anything else:
  stop, it is not validated.
- Nothing was committed after `SHA` that the image carries (`git diff --stat SHA HEAD -- system_files
  build_files apps packaging Containerfile Justfile image-versions.yml` is empty). A later commit touching only
  `tests/`, `plans/`, `.claude/` or docs outside the image does not change what was validated.

## 2. On `main` at GitHub

```bash
git fetch -q origin
git merge-base --is-ancestor SHA origin/main && echo "on main"
```

If `SHA` is not on `origin/main` yet, push exactly it, fast-forward only, never a commit after it:
`git push origin SHA:main`. A refused push (main moved on) is reported, not forced.

## 3. Queue and follow it

```bash
gh run list -R arsslensoft/amethystora --workflow "Latest Images" --limit 3 \
    --json databaseId,status,headSha,createdAt --jq '.[] | "\(.databaseId) \(.status) \(.headSha[0:7]) \(.createdAt)"'
gh workflow run "Latest Images" -R arsslensoft/amethystora --ref main
gh run list -R arsslensoft/amethystora --workflow "Latest Images" --limit 1 --json databaseId,headSha --jq '.[0]'
gh run watch RUN_ID -R arsslensoft/amethystora --exit-status     # run_in_background, the longest timeout
gh run view RUN_ID -R arsslensoft/amethystora --log-failed | tail -n 200   # when it fails
```

- Skip the dispatch when a `Latest Images` run of the same `SHA` is already queued or in progress; follow
  that one instead. A run of an older commit still in progress is left alone (its concurrency group decides).
- The dispatch builds `main`'s head: check the new run's `headSha` is `SHA` (or a later commit that only
  changed files outside the image).

## 4. Report

The run's link, its jobs as they end (each image and flavor), and, on a failure, the failing job, step and the
end of its log. A failure that the lab did not show (an NVIDIA flavor, the sealed image, a registry) is
diagnosed like a build failure in `run-tests` and fixed there; the fixed commit goes through the lab again
before it is queued.
