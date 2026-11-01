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

## Die Protokolle

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

## Eine Aktualisierung hat etwas kaputtgemacht

Wählen Sie im Bootmenü das vorherige System, um zu bestätigen, dass es an der Aktualisierung lag.
Systemaktualisierungen bietet dann an, es zu behalten, was auch `sudo bootc rollback` tut, bis ein
korrigiertes Abbild erscheint. [Aktualisierungen](updates.md#rolling-back).

## Häufige Probleme

### Ein Paket, das ich überlagert habe, ist weg

Es wurde mit `rpm-ostree install` überlagert, und der Rechner wurde danach mit `bootc switch` gewechselt,
das das neue System allein aus dem Abbild baut. Installieren Sie es erneut, und wechseln Sie Abbilder oder
Kanäle ab jetzt mit `rpm-ostree rebase` ([Software installieren](software.md#layering-the-last-resort)).

### Eine App sieht einen Ordner oder ein Gerät nicht, oder öffnet sich gar nicht

Flatpak-Apps erreichen nur, was ihnen erlaubt ist, und unter Amethystora schließt das X11 und
Eingabegeräte aus. Öffnen Sie **Flatseal**, wählen Sie die App und gewähren Sie, was sie braucht: einen
Ordner unter **Dateisystem** oder **X11-Fenstersystem** für eine App, die kein Wayland spricht.

### Eine Erweiterung benimmt sich daneben, oder der Desktop sieht falsch aus

Schalten Sie die Erweiterung in der **Erweiterungsverwaltung** aus, und melden Sie sich dann ab und wieder
an. Wenn der Desktop sich weiterhin daneben benimmt, setzen Sie GNOMEs Einstellungen auf die Vorgaben des
Abbilds zurück und wenden Sie Ihr Thema erneut an:

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Das setzt jede GNOME-Einstellung dieses Kontos zurück, einschließlich des Docks, Ihrer eigenen
Tastenkürzel und Ihrer Auswahl an Erweiterungen; heben Sie es sich also für den Fall auf, dass nichts
anderes geholfen hat.

### `Super+Ctrl+Space` ändert nichts

Es wechselt zum nächsten Hintergrundbild des aktuellen Themas. Ein eigenes Thema mit nur einem Bild hat
nichts, wohin es wechseln könnte, und eine Benachrichtigung sagt das: Fügen Sie weitere hinzu
([Themen](themes.md#wallpapers)). Die Themenauswahl liegt auf `Super+Ctrl+Shift+Space`.

### Das Konto ist nach falschen Passwörtern gesperrt

Zehn falsche Passwörter sperren das Konto für zehn Minuten. Warten Sie, oder entsperren Sie es von einem
anderen Administratorkonto aus mit `sudo faillock --user <name> --reset`.

### Ein DisplayLink-Dock zeigt nichts an

Bei eingeschaltetem Secure Boot lädt sein Treiber erst, wenn der Schlüssel von Amethystora registriert ist:
`ame security secure-boot`, dann neu starten ([Hardware](hardware.md#secure-boot)).

### Tastaturbelegungen wechseln

`Super+Space` ist hier die Suche, deshalb liegt die nächste Tastaturbelegung auf `Super+Shift+Space`.
Fügen Sie Belegungen unter **Einstellungen → Tastatur** hinzu.

### Die Festplatte läuft voll

```bash
ame system clean
```

Entfernt ungenutzte Container, Abbilder und Flatpak-Laufzeitumgebungen. `flatpak uninstall --unused` und
`podman system prune` erledigen Teile davon von Hand.

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
