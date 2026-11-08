# Notizen und Aufgaben

**Notizen** in der Anwendungsübersicht bewahrt Ihre Notizen und Ihre To-do-Listen in einem einzigen
Fenster auf. Alles bleibt auf diesem Rechner. Nichts wird synchronisiert, und die App verbindet sich nie
mit dem Internet. Sie ersetzt Joplin und Planify und übernimmt, was Sie dort bereits haben (siehe
[Von Joplin oder Planify umsteigen](#coming-from-joplin-or-planify)).

Beim ersten Öffnen bietet sie an, Ihre Notizen mit einer Passphrase zu verschlüsseln. Diese Wahl empfiehlt
sie, und der Rest dieser Seite geht davon aus, dass Sie sie getroffen haben.

## Notizen

Notizen werden in Markdown geschrieben und liegen in Notizbüchern, die weitere Notizbücher enthalten
können. Der Editor zeigt den Text und die formatierte Notiz nebeneinander. **Ctrl+E** wechselt zwischen
dem Text, beidem und der formatierten Notiz allein.

- **Checklisten:** `- [ ] like this`. Haken Sie die Kästchen in der formatierten Notiz ab.
- **Anhänge:** Fügen Sie ein Bild ein, ziehen Sie eine Datei auf die Notiz oder verwenden Sie die
  Büroklammer. Bilder werden in der Notiz angezeigt. Klicken Sie auf einen anderen Anhang, um eine Kopie
  zu speichern.
- **Schlagwörter:** Tippen Sie in die Schlagwortzeile unter dem Titel und drücken Sie **Enter**.
  Schlagwörter werden in der Seitenleiste aufgeführt.
- **Eine Notiz verschieben:** Wählen Sie oben in der Notiz ein anderes Notizbuch, oder ziehen Sie die
  Notiz auf ein Notizbuch in der Seitenleiste.
- **Frühere Versionen:** die Uhr oben in einer Notiz. Jedes Mal, wenn Sie eine Notiz nach zehn Minuten
  Pause wieder bearbeiten, wird eine Version aufbewahrt, bis zu den letzten 30. Wenn Sie eine
  wiederherstellen, bleibt der Text, den sie ersetzt, ebenfalls als Version erhalten.
- **Papierkorb:** Eine Notiz, die Sie löschen, kommt in den Papierkorb, wo Sie sie wiederherstellen oder
  endgültig löschen können.
- **PDF:** Das Menü **⋯** einer Notiz exportiert sie als PDF.

## Aufgaben

**Aufgaben** oben in der Seitenleiste wechselt zu Ihren To-do-Listen:

| Ansicht | Zeigt |
| --- | --- |
| Eingang | Aufgaben, die in keinem Projekt sind |
| Heute | Was heute fällig ist, und alles Überfällige |
| Demnächst | Die kommende Woche, Tag für Tag, und alles danach |
| Angeheftet | Die Aufgaben, die Sie angeheftet haben |
| Erledigt | Was Sie erledigt haben, das Neueste zuerst |

Projekte gruppieren Aufgaben und lassen sich in Abschnitte unterteilen. Labels gelten projektübergreifend.
Klicken Sie auf eine Aufgabe, um Datum und Uhrzeit, Wiederholung, Erinnerung, Priorität, Labels und
Notizen festzulegen und Unteraufgaben hinzuzufügen.

Das Feld **Aufgabe hinzufügen** versteht ein wenig Alltagssprache, allerdings auf Englisch. Es nimmt
Folgendes aus dem Titel heraus:

| Eingabe | Für |
| --- | --- |
| `today`, `tomorrow`, `friday`, `next week`, `2026-10-31` | Das Datum |
| `5pm`, `17:30` | Die Uhrzeit, mit einer Erinnerung zu dieser Zeit |
| `every day`, `every 2 weeks`, `monthly` | Eine Aufgabe, die sich wiederholt |
| `p1` bis `p4` | Die Priorität, `p1` die höchste |
| `@errand` | Ein Label, das angelegt wird, falls es neu ist |
| `#Garden` | Ein vorhandenes Projekt |

Wenn Sie eine sich wiederholende Aufgabe erledigen, rückt sie auf ihr nächstes Datum vor.

### Erinnerungen

Eine Aufgabe mit Uhrzeit kann Sie erinnern, von **Zum Zeitpunkt** bis **1 Tag vorher**. Erinnerungen
erscheinen als Benachrichtigungen, während Notizen läuft. Wenn Sie das Fenster schließen, läuft Notizen im
Hintergrund weiter, bis die anstehenden Erinnerungen ausgelöst wurden. Das können Sie in den
**Einstellungen** ausschalten. Ihre Notizen sind gesperrt, während die App so läuft; nur die Titel und
Zeiten der Erinnerungen werden im Arbeitsspeicher behalten.

## Von Joplin oder Planify umsteigen

Öffnen Sie die **Einstellungen** und verwenden Sie **Importieren**. Wenn Sie dieselbe Datei erneut
importieren, wird aktualisiert, was aus ihr stammt, statt es doppelt hinzuzufügen.

- **Joplin:** Wählen Sie in Joplin **Datei**, **Alles exportieren**, **JEX**. Importieren Sie die
  `.jex`-Datei. Notizbücher, Notizen, Schlagwörter und Anhänge werden übernommen. Die To-dos von Joplin
  werden zu Aufgaben im Eingang.
- **Planify:** Öffnen Sie in Planify **Einstellungen**, **Sicherung**, und erstellen Sie eine Sicherung.
  Importieren Sie deren `.json`-Datei aus
  `~/.var/app/io.github.alainm23.planify/data/io.github.alainm23.planify/backups`. Projekte, Abschnitte,
  Aufgaben, Unteraufgaben, Labels, Daten und Wiederholungen werden übernommen.
- **Markdown-Dateien** kommen in ein Notizbuch namens **Imported**.

Sobald alles übernommen ist, können Sie die alten Apps in Bazaar entfernen oder mit
`flatpak uninstall net.cozic.joplin_desktop io.github.alainm23.planify`.

## Tasten

| Taste | Funktion |
| --- | --- |
| **Ctrl+N** | Eine neue Notiz, oder eine neue Aufgabe in **Aufgaben** |
| **Ctrl+F** | Notizen oder Aufgaben durchsuchen |
| **Ctrl+1** / **Ctrl+2** | Notizen / Aufgaben |
| **Ctrl+E** | Text, beides oder die formatierte Notiz |
| **Ctrl+B**, **Ctrl+I**, **Ctrl+K** | Fett, kursiv, Link |
| **Ctrl+L** | Sperren |
| **Ctrl+,** | Einstellungen |
