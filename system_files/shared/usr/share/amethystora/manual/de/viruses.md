# Viren und Ransomware

## Nach Viren suchen

**Virenscan** in der App **Sicherheit** (oder `amethystora-security scan`) scannt, wann immer Sie es
wünschen:

- **Ihr persönlicher Ordner**, außer Caches.
- **Eine Datei oder ein Ordner**: Wählen Sie sie aus, oder ziehen Sie sie an eine beliebige Stelle des
  Fensters.
- **Der ganze Rechner**: alle persönlichen und temporären Ordner, wie beim wöchentlichen Scan. Er fragt
  nach keinem Passwort und läuft weiter, wenn Sie das Fenster schließen. Was er findet, wird dort
  aufbewahrt, wo nur ein Administrator es lesen kann, daher fragt das Ansehen der Funde sehr wohl nach
  Ihrem Passwort.

Ein Scan, den Sie starten, läuft als Sie und prüft also, was Ihr Konto lesen kann. Standardmäßig wird
nichts, was er findet, gelöscht, unter Quarantäne gestellt oder verschoben: Das Ergebnis listet jede Datei
auf, mit **In Dateien anzeigen**, und überlässt Ihnen die Entscheidung. Ein Treffer kann ein Fehlalarm
sein, prüfen Sie also, woher eine Datei stammt, bevor Sie sie löschen, und scannen Sie dann erneut. Wenn
Sie Scans auf Quarantäne oder Löschen eingestellt haben, fragt die App nach Ende des Scans nach Ihrem
Passwort und sagt, was mit jeder Datei geschehen ist. Die Scans, die Sie ausgeführt haben, sind in
`~/.local/state/amethystora/security-scans.json` aufgeführt.

## Ransomware-Schutz

```bash
ame security ransomware
```

Jede Stunde ein schreibgeschützter Schnappschuss jedes persönlichen Ordners auf diesem Rechner,
aufbewahrt in `/var/home/.snapshots`. Ransomware, die unter Ihrem Konto läuft, kann alles verschlüsseln,
was Sie schreiben können, und ein Schnappschuss ist nichts, was Sie schreiben können: Ihn zu ändern oder
zu löschen erfordert einen Administrator. Jeder Schnappschuss des letzten Tages wird aufbewahrt, und zwei
Wochen lang einer pro Tag. Der Bericht meldet, wenn der neueste älter als ein paar Stunden ist.

Um Dateien zurückzuholen, führen Sie `ame security ransomware restore` aus und wählen den Zeitpunkt, zu dem
Sie zurückkehren möchten. **Open it in Files** zeigt Ihren persönlichen Ordner, wie er damals war, zum
Herauskopieren; **Put a folder back** kopiert einen ganzen Ordner an seinen Platz zurück, ersetzt die
Dateien, die er enthält, und lässt die übrigen in Ruhe, etwa was Ransomware umbenannt hat. Die Dateien
anderer sind in einem Schnappschuss nicht sichtbar, und Ihre sind für sie nicht sichtbar.

Er braucht die persönlichen Ordner auf einem eigenen btrfs-Subvolume, so wie das Installationsprogramm
eine Festplatte einrichtet, sofern sie nicht von Hand partitioniert wurde. Der Preis ist Speicherplatz: Ein
Schnappschuss hält fest, was seitdem geändert oder gelöscht wurde, sodass eine Datei, die Sie löschen, um
Platz zu schaffen, ihren Platz erst freigibt, wenn der letzte Schnappschuss verschwindet, der sie enthält,
bis zu zwei Wochen später. Schnappschüsse werden nie gelöscht, um Platz zu schaffen, weil Ransomware, die
jede Datei umschreibt, sonst die von vorher löschen würde. Beim Ausschalten wird angeboten, sie zu löschen.

Schnappschüsse liegen auf derselben Festplatte, und Ransomware, die Administratorrechte erlangt, kann sie
löschen. Bewahren Sie zusätzlich eine [Sicherung](backups.md) an einem anderen Ort auf.
