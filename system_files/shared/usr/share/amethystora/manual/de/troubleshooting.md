# Fehlerbehebung

Die meisten Probleme auf einem abbildbasierten System haben dieselben drei Antworten: herausfinden, was
passiert ist, zurückkehren, wenn eine Aktualisierung es verursacht hat, und es melden, damit das nächste
Abbild es behebt.

## Zuerst den Agenten nachsehen lassen

**Diagnose a problem** im Amethystora-Menü (`Super+Alt+Space`) oder in einem Terminal:

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

Er liest die Protokolle und Absturzberichte, ändert nichts und meldet sich mit der Ursache und einer
Lösung, die Sie in Betracht ziehen können. [Der KI-Agent](ai-agent.md#diagnose-a-problem) erklärt mehr.

## Eine Aktualisierung hat etwas kaputtgemacht

Wählen Sie im Bootmenü das vorherige System, um zu bestätigen, dass es an der Aktualisierung lag.
Systemaktualisierungen bietet dann an, es zu behalten, was auch `sudo bootc rollback` tut, bis ein
korrigiertes Abbild erscheint. [Aktualisierungen](rolling-back.md).

## Ein Problem melden

Melden Sie Fehler auf [GitHub](https://github.com/arsslensoft/amethystora/issues) und stellen Sie Fragen in
den [Diskussionen](https://github.com/arsslensoft/amethystora/discussions). `ame report` sammelt, was ein
Bericht braucht, in einer Datei in Ihrem persönlichen Ordner (das Abbild, Ihre Gruppen, fehlgeschlagene
Dienste und die seit diesem Systemstart protokollierten Fehler), zeigt Ihnen alles davon und öffnet dann,
wenn Sie zustimmen, ein neues Issue, in dem alles außer den Fehlern ausgefüllt ist. Nichts wird
veröffentlicht, bevor Sie es absenden; ziehen Sie die Datei hinein, wenn die Fehler helfen. Andernfalls
geben Sie an:

- die Ausgabe von `rpm-ostree status`, die das genaue Abbild nennt,
- was Sie getan haben, was Sie erwartet haben und was stattdessen passiert ist,
- die relevanten Zeilen aus den oben genannten Protokollen,
- ob das vorherige Abbild dasselbe Problem hatte.
