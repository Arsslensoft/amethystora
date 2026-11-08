# Sicherungen

```bash
ame backup
```

Eine tägliche restic-Sicherung in ein Repository, aus dem Ransomware auf diesem Rechner nichts löschen
kann. Die Sicherung fügt dem Repository immer nur etwas hinzu und löscht nie alte Schnappschüsse, sodass
ein Repository am anderen Ende, das nur Anhängen erlaubt, alles behält, was vor einem Angriff gesichert
wurde. Der Befehl erklärt, wie Sie eines auf rest-server, Borg, einem Objektspeicher mit
Unveränderlichkeit oder einem USB-Laufwerk einrichten, das Sie zwischen den Sicherungen abziehen.
`ame backup now` sichert sofort, und `ame backup snapshots` listet auf, was vorhanden ist.

Jede Sicherung bewahrt neben Ihren Dateien auch Ihre Einrichtung auf: das Abbild und den Kanal, dem Sie
folgen, Ihre Flatpak-Apps und woher sie stammen, Ihre Homebrew-Pakete, die Container, die `amepkg`
erstellt hat, mit dem, was Sie darin installiert haben, die GNOME-Erweiterungen, die Sie eingeschaltet
hatten, und Ihr Thema. Keine Passwörter, Schlüssel oder Token gelangen hinein. Richten Sie auf einem neuen
Rechner `ame backup` mit demselben Passwort auf dasselbe Repository aus, und dann:

```bash
ame restore-setup
```

Es zeigt zuerst, was es tun würde, und erledigt nur die Teile, die Sie auswählen: zum selben Abbild
wechseln, über `ame system rebase`, das nur signierte Abbilder annimmt; die Apps und Homebrew-Pakete
installieren; die Container neu erstellen und installieren, was darin war; die Erweiterungen wieder
einschalten; das Thema setzen. `ame restore-setup <snapshot>` wählt eine ältere Einrichtung. Ihre Dateien
selbst holen Sie mit restic zurück.
