# App-Berechtigungen

Jede Flatpak-App läuft in einer Sandbox, und ihre Berechtigungen sagen, wie weit die App ohne Nachfrage
darüber hinaus zugreifen kann. Die Seite **Apps** der Sicherheits-App und `ame security apps` in einem
Terminal listen für jede App auf, worauf sie zugreifen kann, und sagen, was davon auf diesem Rechner
gewährt wurde, statt von der App selbst verlangt:

| Was sie tun kann | Was das bedeutet |
| --- | --- |
| Mit jedem Dienst Ihrer Sitzung oder des Systems kommunizieren, den Flatpak-Dienst oder das systemd Ihrer Sitzung verwenden oder ein Programm dort ablegen, wo es bei Ihrer nächsten Anmeldung startet | Sie kann ihre Sandbox verlassen: Was darin läuft, kann alles, was Sie können |
| X11 verwenden oder die Eingabegeräte lesen | Sie kann sehen, was Sie in andere Apps eingeben. Das Abbild entzieht beides jeder App, also hat eine App es nur, wenn es ihr zurückgewährt wurde |
| Alle Geräte verwenden | Kameras, Sicherheitsschlüssel und Gamecontroller, ohne zu fragen |
| Ihre Dateien lesen und ändern oder nur lesen | Alles in Ihrem persönlichen Ordner oder jede Datei, die Sie öffnen können |
| Ihre SSH- oder GPG-Schlüssel verwenden oder Ihren Schlüsselbund lesen | Sich als Sie anmelden, signieren oder entschlüsseln oder jedes gespeicherte Passwort lesen |

Was eine App über ein Portal anfragt, etwa eine Datei, die Sie für sie auswählen, wird jedes Mal erfragt
und ist nicht aufgeführt. Der Sicherheitsbericht meldet, wenn eine App ihre Sandbox verlassen kann und
wenn einer X11 oder die Eingabegeräte zurückgegeben wurden. Wenn Sie das nicht beabsichtigt haben, ändern
Sie es in **Flatseal**, oder nehmen Sie alles, was Sie einer App gewährt haben, mit
**Gewährtes zurücknehmen** auf der Seite „Apps“ zurück, oder:

```bash
ame security apps reset
```

## Dateien teilen, ohne dass sie etwas über Sie verraten

Fotos, Dokumente und Aufnahmen tragen mehr in sich, als sie zeigen: wo ein Foto aufgenommen wurde und mit
welchem Telefon, wer ein Dokument geschrieben hat und wann. **Metadata Cleaner** zeigt, was eine Datei
über Sie verrät, und speichert eine Kopie ohne diese Angaben. Öffnen Sie es in der Anwendungsübersicht und
fügen Sie die Dateien hinzu, bevor Sie sie versenden oder veröffentlichen.
