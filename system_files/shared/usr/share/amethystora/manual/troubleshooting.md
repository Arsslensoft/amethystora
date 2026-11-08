# Troubleshooting

Most problems on an image-based system have the same three answers: find out what happened, roll back if
an update caused it, and report it so that the next image fixes it.

## Let the agent look first

With the [AI agent](ai-agent.md) on (`ame agent toggle on`), choose **Diagnose a problem** in the
Amethystora menu (`Super+Alt+Space`), or in a terminal:

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

It reads the logs and crash reports, changes nothing, and comes back with the cause and a fix to consider.
[The AI agent](ai-agent.md#diagnose-a-problem) has more.

### The health check

Without the agent, `ame system doctor` runs the same health check it starts from: failed services,
updates, disk space, memory, crashes, the clock, the driver, backups and pools of disks, each finding with
what fixes it. It only reads and needs no password. `ame system doctor ID`, with the id a finding named,
checks that one again after a fix.

## When something is wrong

Press **Something's wrong** in the Amethystora menu at the left of the top bar (`amethystora-help`), and
pick what it is in plain words: no sound, no internet, the printer, it feels slow, running out of room, a
device that does nothing, or something else. It looks, says what it found in a sentence, and offers one
thing to press, with the `ame` command it runs written under it. [Show me](guide:something-wrong)

### What it reads, and what it may do

- **Until you press, it only reads,** with one exception: for **No internet**, unless NetworkManager's
  own check says the internet works, it sends your router one test message, and once that check has
  failed it looks up the one name the check uses (never while the check is off, and never a name of
  Amethystora's own).
- **It says what it saw, not more:** a router that does not answer a test message is not called broken.
- **Security is switched in Security:** anything to do with a security feature only opens it.
- **A fix that asks something, needs a password, or would end what is running** (every connection, or
  every sound, a call included) opens in a terminal and says so first, and is never offered when nothing
  is wrong.

### What changed

**What changed**, in the same menu and window (`amethystora-help changed`), lists what changed on this
computer in the last month, in plain words: a file you deleted, an app you removed, a setting, an update.
Nothing keeps a record for it: the list is worked out from what the computer already has when you open it,
and is gone when you close it.

### Undo

**Undo** adds back and never deletes:

- **A file** comes out of the trash to where it was, or beside what now has its name, when that is in
  your own files and not a place that starts programs or holds keys; Files puts it back from anywhere else.
  A file gone from your usual folders and not in the trash comes back from an hourly snapshot, if the
  snapshots are on and one held it.
- **An app** is installed again from where it came from, in a terminal, and finds what it had saved. An app
  you installed is listed with no Undo: remove it as any app.
- **A setting** has its old value again, from the snapshot from before it changed. Only the settings a
  person would miss are listed.
- **An update** goes back to the version before it, at the next restart, in a terminal and with an
  administrator's password.
- **A reset of the settings, and a change the AI agent made** while its tools may make changes, are undone
  as each is anywhere else. An app the agent installed has no Undo here: removing is not adding back, so
  remove it as any app.

A security setting's line has no Undo: it opens Security.

## An update broke something

Pick the previous system in the boot menu to confirm it was the update. System Updates then offers to keep
it, as `sudo bootc rollback` does, until a fixed image is out. [Updates](rolling-back.md).

## Reporting a problem

Report bugs on [GitHub](https://github.com/arsslensoft/amethystora/issues), and ask questions in the
[discussions](https://github.com/arsslensoft/amethystora/discussions), which **Community** in the app grid
opens.

### What `ame report` gathers

`ame report` gathers what a report needs into a file in your home folder: the image and its deployments,
the kernel, the machine's maker and model, what the [driver check](hardware.md#when-a-device-does-not-work)
found, your groups, failed services and the errors logged since this boot. It shows you all of it, then, if
you say so, opens a new issue with all but the errors filled in. Nothing is posted until you submit it;
drag the file in if the errors help.

Otherwise, include:

- the output of `rpm-ostree status` (`sudo bootc status` on a sealed machine), which names the exact image,
- what you did, what you expected, and what happened instead,
- the relevant lines from the logs above,
- whether the previous image had the same problem.
