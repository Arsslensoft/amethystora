# Die Protokolle

Öffnen Sie **Protokolle** in der Anwendungsübersicht, oder führen Sie `amethystora-logs` aus. Die App
öffnet mit **Wichtig**, den Fehlern von allem auf dem Rechner seit seinem Start. Die Seitenleiste teilt den
Rest auf in Ihre Sitzung, das System, Kernel und Hardware, Sicherheit (Anmeldungen, `sudo`, was die
Audit-Regeln erfasst haben) und **Abstürze**, jeweils mit einer Zahl dessen, was seit dem letzten
Systemstart neu ist, wo es darauf ankommt.

- **Suche** durchsucht beim Tippen, was die Meldungen sagen. Der Knopf `.*` daneben behandelt die Suche
  stattdessen als regulären Ausdruck.
- **Zeitraum** geht zurück zum vorherigen Systemstart, zu jedem Systemstart, den das Journal noch
  aufbewahrt, zur letzten Stunde, zum letzten Tag oder zur letzten Woche, oder zum gesamten Journal.
  **Stufe** blendet aus, was weniger ernst ist als Ihre Auswahl.
- **Apps und Dienste** listet alles auf, was ins Journal geschrieben hat. Wählen Sie eine App, um zu
  lesen, was sie bei jedem Start gesagt hat, oder einen Dienst des Systems oder Ihrer Sitzung.
- Klicken Sie auf einen Eintrag, um ihn vollständig zu lesen, mit jedem Feld, das das Journal speichert.
  **Nur …** beschränkt die Ansicht auf seine Herkunft, und **Davor und danach** zeigt alles, was in der
  Minute davor und danach protokolliert wurde.
- **Verfolgen** fügt neue Einträge hinzu, sobald sie geschrieben werden. **Exportieren** speichert das
  Angezeigte in einer Text- oder JSON-Datei, und der Terminal-Knopf öffnet dieselbe Ansicht in
  `journalctl`. Der Befehl für die Ansicht steht auch unten in der Seitenleiste.

Administratoren, die Konten in `wheel`, sehen alles. Jedes andere Konto sieht nur die eigene Sitzung und
die eigenen Apps, und die Ansichten des Systems weisen darauf hin.

Dasselbe in einem Terminal:

| Befehl | Zeigt |
| --- | --- |
| `journalctl -b -p warning` | Warnungen und Fehler seit diesem Systemstart |
| `journalctl -b -1 -p warning` | Dasselbe für den vorherigen Systemstart, nach einem Absturz oder Einfrieren |
| `journalctl --user -b` | Ihre eigene Sitzung: der Desktop, Ihre Apps |
| `systemctl --failed` | Dienste, die nicht starten konnten |
| `coredumpctl list` | Programme, die abgestürzt sind |
| `ame system logs-this-boot` | Alles seit diesem Systemstart |
| `ame system local-overrides` | Die Dateien in `/etc`, die Sie oder etwas, das Sie ausgeführt haben, geändert haben |
