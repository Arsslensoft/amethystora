# Notes and tasks

**Notes** in the app grid keeps your notes and your to-do lists in one window. Everything stays on this computer:
nothing is synced, and the app never connects to the internet. It takes the place of Joplin and Planify, and brings in
what you have in them (see [Coming from Joplin or Planify](#coming-from-joplin-or-planify)).

The first time you open it, it offers to [encrypt your notes](notes-encryption.md) with a passphrase. That is the choice
it recommends, and this page assumes you took it. Passwords, authenticator codes and keys do not belong in a note: keep
them in the [Vault](vault.md), which asks before each use.

## Notes

Notes are written in Markdown and sit in notebooks, which can hold other notebooks. The **+** beside **Notebooks**
makes one, and a notebook's **More** menu adds a note or a notebook inside it, renames it, moves it to the top level, or
deletes it, which sends its notes to the trash. The sort button over the list orders the notes by when they last
changed, when they were made, or by title.

### Writing

The editor shows the text and the formatted note side by side, or one above the other in a narrow window. **Ctrl+E**,
or the three buttons at the top of the note, switch between the text, both, and the formatted note alone.

The toolbar above the text makes a selection bold or italic, a heading, a list, a checklist, a quote, code or a link,
and takes a heading or list off again. `Tab` and `Shift+Tab` indent and outdent, and `Enter` carries a list on, or ends
it on an empty item. Links to web pages and mail addresses open in their apps.

### Checklists, attachments and the rest

- **Checklists:** `- [ ] like this`. Tick the boxes in the formatted note.
- **Attachments:** paste an image, drop a file on the note, or use the paperclip, up to 200 MB each.
  Pictures show in the note, from attachments only, never from a file elsewhere on the computer. Click
  any other attachment to save a copy, which is not encrypted.
- **Tags:** type in the tag row under the title and press **Enter** or a comma; it offers the tags
  you have, and `Backspace` in the empty row takes the last one off. Tags are listed in the sidebar,
  where a tag's **More** menu renames it or takes it off every note.
- **Moving a note:** pick another notebook at the top of the note, or drag the note onto a notebook
  in the sidebar.
- **Earlier versions:** the clock at the top of a note. A version is kept each time you come back to
  change a note after ten minutes away from it, up to the last 30. Restoring one keeps the text it
  replaces as a version too.
- **Trash:** **Move to the trash**, in a note's **⋯** menu, can be undone at once. In the trash a note
  can be restored or deleted for good, with its earlier versions and attachments, and **Empty** deletes
  them all. Nothing leaves the trash by itself.
- **PDF:** the **⋯** menu of a note exports it as a PDF, with its pictures.

## Tasks

**Tasks** at the top of the sidebar switches to your to-do lists, with the number due today or overdue;
`amethystora-notes tasks`, or **Tasks** in the right-click menu of Notes, opens them straight away.

![Tick off tasks](media/notes-tick-tasks.webm)

| View | Shows |
| --- | --- |
| Inbox | Tasks that are not in a project |
| Today | What is due today, and anything overdue |
| Upcoming | The week ahead, day by day, and everything after it |
| Pinned | The tasks you pinned |
| Completed | What you finished, most recent first |

### Projects, sections and labels

Projects group tasks and can be divided into sections; labels cut across projects. The **+** beside **Projects** or
**Labels** makes one, and its **More** menu renames it, gives it a colour, adds a section to a project, or deletes it.
**Deleting a project deletes its tasks**; deleting a section leaves its tasks in the project, and deleting a label only
takes it off its tasks.

Click a task to move it to another project or section, set its date and time, repeat, reminder, priority, labels and
notes, add subtasks, pin it, or delete it, which can be undone at once. Completing a task completes its subtasks too,
and a project folds its completed tasks away.

### Adding a task in plain words

The **Add a task** field understands a little plain language. It takes these out of the title:

| Type | For |
| --- | --- |
| `today`, `tonight`, `tomorrow` (`tmr`), `friday` or `on fri`, `next week` (next Monday), `2026-10-31` | The date |
| `5pm`, `at 17:30` | The time, with a reminder at that time; on today, or tomorrow once it has passed, when no date is given |
| `every day`, `every 2 weeks`, `daily`, `weekly`, `monthly`, `yearly` | A task that repeats |
| `p1` to `p4` | The priority, `p1` the highest |
| `@errand` | A label, made if it is new |
| `#Garden` | A project that exists, its name typed without spaces (`#GardenWork`) |

Completing a repeating task moves it to its next date. The words are English whatever the language of
the window.

### A quick note

`Super+Alt+N` (or `amethystora-notes capture`) opens a small window over anything. `Enter` saves what you typed as a
task in the inbox, with the dates and the rest the task box understands (its first line is the task, the lines after
it its notes); `Shift+Enter` starts a new line and `Esc` closes without saving. When the notes are locked, it asks for
the passphrase (or a passkey) first, and locks them again after unless Notes' window is open. The text is never written
anywhere until you save it.

From the search (`Super+Space`), `remind me at 17:00 to call the garage` makes the same task, with its
reminder, when you press `Enter` ([Finding things](search.md)).

### Reminders

A task with a time can remind you, from **At the time** to **1 day before**. Reminders appear as notifications while
Notes is running, and clicking one opens the task. When you close the window, Notes keeps running in the background
until the reminders still to come have gone off; **Keep running for reminders**, in **Settings**, turns that off. Your
notes are locked while it runs like that; only the titles and times of the reminders are kept, in memory.

## Coming from Joplin or Planify

Open **Settings** and use **Import**. Importing the same file again updates what came from it rather
than adding it twice.

- **Joplin:** in Joplin, choose **File**, **Export all**, **JEX**. Import the `.jex` file.
  Notebooks, notes, tags and attachments come across. Joplin's to-dos become tasks in the inbox.
- **Planify:** in Planify, open **Preferences**, **Backup**, and create a backup. Import its `.json`
  file from `~/.var/app/io.github.alainm23.planify/data/io.github.alainm23.planify/backups`. Projects,
  sections, tasks, subtasks, labels, dates and repeats come across.
- **Markdown files** go into a notebook called **Imported**.

Once everything is across, you can remove the old apps in Bazaar, or with
`flatpak uninstall net.cozic.joplin_desktop io.github.alainm23.planify`.

## Keys

| Key | Does |
| --- | --- |
| **Ctrl+N** | A new note, or a new task in **Tasks** |
| **Ctrl+F** | Search notes or tasks |
| **Ctrl+1** / **Ctrl+2** | Notes / Tasks |
| **Ctrl+E** | Text, both, or the formatted note |
| **Ctrl+B**, **Ctrl+I**, **Ctrl+K** | Bold, italic, link |
| **Ctrl+L** | Lock, while the notes are encrypted |
| **Esc** | Close a menu, or the open task |
| **Ctrl+,** | Settings |
| **Super+Alt+N** | A quick note into the inbox, from anywhere |
