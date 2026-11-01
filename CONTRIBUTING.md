# CONTRIBUTING

Thanks for helping out!

This repository builds the Amethystora images. Open an issue or pull request on [arsslensoft/amethystora](https://github.com/arsslensoft/amethystora).

Amethystora is based on [Universal Blue](https://universal-blue.org). Its desktop layer lives in this repository, under `system_files/shared`, and is not pulled from upstream, so changes to it are made here.

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).

## Translating

Amethystora's own apps, the security report and the manual are meant to be translated on Weblate, but
the project there is not set up yet. Until it is, send a translation as a pull request that adds or
changes only the files of your language, in the places below.

What there is to translate, and how each is to be set up on Weblate:

| What | Weblate file format | File mask | Base or template file |
| --- | --- | --- | --- |
| Each app: `manual`, `security`, `update`, `logs`, `notes`, `control`, `backups` | JSON file (monolingual), with the flag `icu-message-format` | `system_files/shared/usr/lib/amethystora-<app>/resources/app/locale/*.json` | `.../locale/en.json` |
| The security report, the app permissions, the `ame` commands' prompts and messages, the notifications and the top bar widgets' menu | gettext PO file, with the "Update PO files to match POT (msgmerge)" add-on | `po/*.po` | `po/amethystora.pot` |
| The manual's pages | Markdown file, one component per page, found by the "Component discovery" add-on with `system_files/shared/usr/share/amethystora/manual/(?P<language>[^/]+)/(?P<component>[^/]+)\.md` | `system_files/shared/usr/share/amethystora/manual/*/<page>.md` | `system_files/shared/usr/share/amethystora/manual/<page>.md` |
| The hotkeys page, set up by hand because its English is one folder up | Markdown file | `system_files/shared/usr/share/amethystora/manual/*/keybindings.md` | `system_files/shared/usr/share/amethystora/keybindings.md` |

A few things to keep in mind when translating:

- The manual's `whats-new.md` is written by each build from its changes and is never translated: leave it out of
  Weblate (the build fails on a `manual/<language>/whats-new.md`).
- The apps' messages are ICU MessageFormat: keep `{name}` as it is, and give a plural every form your
  language has (`{count, plural, one {...} few {...} other {...}}`).
- The report's messages, the `ame` commands' and the notifications' are shell text: keep `${NAME}` as it is. A notification is translated for each account, in the language its owner chose, whichever service sent it. The states `ok`, `check` and `off` sit
  in a column seven letters wide.
- In the manual, keep commands, paths, key names and the targets of links (`updates.md#rolling-back`)
  exactly as they are: only what people read changes.

When the English changes, run `node build_files/shared/check-translations.js --write --po po
system_files/shared`, which rewrites each app's `locale/en.json` and `po/amethystora.pot` from the code,
and commit them with the change. The build runs the same check and fails on a catalog that does not
match the code.
