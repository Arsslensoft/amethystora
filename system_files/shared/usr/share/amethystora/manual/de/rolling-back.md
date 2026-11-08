# Rollback

Das vorherige System wird immer aufbewahrt. Wenn eine Aktualisierung ein Problem mitbringt:

- **Im Bootmenü** wählen Sie den zweiten Eintrag, das System vor der Aktualisierung. Nichts wird
  geändert, und der nächste Neustart kehrt zum neuesten zurück. Eine Benachrichtigung bei der Anmeldung
  sagt, dass Sie die vorherige Version verwenden, und ein Klick darauf öffnet Systemaktualisierungen, das
  Ihnen **Diese Version behalten** anbietet, was sie zu der Version macht, die der Rechner startet, und
  nach Ihrem Passwort fragt, oder **Mit der neuesten neu starten**.
- **Um beim vorherigen System zu bleiben**, machen Sie es in einem Terminal zur Vorgabe und starten neu:

```bash
sudo bootc rollback
systemctl reboot
```

Ihre Dateien und Einstellungen sind in beiden dieselben. Die nächste Aktualisierung bringt die neueste
Version zurück, ob automatisch oder nicht; um also zu bleiben, wo Sie sind, bis eine Korrektur erscheint,
schalten Sie in Systemaktualisierungen die automatischen Aktualisierungen aus oder
[bleiben Sie bei dem Build](#holding-on-to-one-build), auf dem Sie sind.
[Melden Sie dann das Problem](troubleshooting.md#reporting-a-problem), und folgen Sie wieder dem Kanal,
sobald ein korrigiertes Abbild erscheint.

## Bei einem Build bleiben

Jeder Build hat auch ein eigenes Tag, etwa `stable-44.20260922`, nach der Version in den
Versionshinweisen. Wenn Sie zu ihm wechseln, bleibt der Rechner auf diesem Build, was nützlich ist,
während Sie auf eine Korrektur warten. Ein Rechner auf einem einzelnen Build erhält keine Aktualisierungen;
`ame security status` erinnert Sie daran und gibt den Befehl aus, mit dem Sie wieder dem Kanal folgen.
