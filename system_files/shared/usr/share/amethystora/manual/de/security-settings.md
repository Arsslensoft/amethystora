# Einstellungen

```bash
ame security settings
```

- **Was mit den Funden eines Virenscans geschieht**: `report` (die Vorgabe) lässt sie, wo sie sind,
  `quarantine` verschiebt sie nach `/var/lib/amethystora/quarantine`, wo nichts sie öffnen oder ausführen
  kann, und `delete` löscht sie. Das gilt für den wöchentlichen Scan, die Scans, die Sie in der App
  starten, und die Echtzeitprüfung. Jede Datei wird erneut geprüft, bevor irgendetwas mit ihr geschieht.
  `ame security scan quarantine` listet auf, was in Quarantäne ist, und `ame security scan restore` stellt
  eine Datei wieder her und kann den Scanner anweisen, genau diese Datei künftig in Ruhe zu lassen.
- **Echtzeitüberwachung**: `off` (die Vorgabe) oder `on`. Eingeschaltet liest der Sicherheitswächter jede
  Änderung, sobald das Audit-Protokoll sie aufzeichnet, und jede in einem persönlichen Ordner geschriebene
  Datei wird beim Schließen gescannt. Das kostet etwas Prozessorleistung, während Dateien geschrieben
  werden, und das Scannen beginnt, sobald die ersten Virensignaturen heruntergeladen sind.
- **Netzwerkschutz**: `off` (die Vorgabe), `watch` oder `block`.
  [Netzwerkschutz](network-security.md#network-protection).

Alle drei stehen in `/etc/amethystora/security.conf`. Der Befehl wendet eine Änderung sofort an; eine
Bearbeitung der Datei von Hand gilt ab dem nächsten Neustart.
