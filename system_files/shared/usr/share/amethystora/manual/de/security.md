# Sicherheit

Amethystora ist über die Vorgaben von Fedora hinaus gehärtet und sagt das offen: Alles hier unten lässt
sich überprüfen, und das meiste lässt sich ändern, wenn es Ihnen im Weg ist.

## Sehen, wo Sie stehen

Öffnen Sie **Sicherheit** in der Anwendungsübersicht oder führen Sie aus:

```bash
ame security status
```

Beides zeigt, was die Sicherheitseinstellungen gerade tatsächlich tun, und was bei allem zu tun ist, das
nicht stimmt, und keines von beiden ändert etwas oder fragt nach einem Passwort. In der App steht zuerst,
was Ihre Aufmerksamkeit braucht, mit dem Befehl, der es behebt, und einem Knopf, der ihn in einem Terminal
ausführt, und die Schutzmaßnahmen weiter unten, die ausgeschaltet bleiben, bis Sie sie wollen, sind
jeweils nur einen Knopfdruck entfernt. Für einen genaueren Blick lässt `ame security audit` Lynis einige
hundert Prüfungen durchlaufen und erklärt jeden Fund.

Die Seite **Apps** der App listet auf, worauf jede Flatpak-App über ihre Sandbox hinaus zugreifen kann und
was davon auf diesem Rechner gewährt wurde ([App-Berechtigungen](app-permissions.md)).

Die Seite **Container** der App listet auf, was außerhalb von Flatpak installiert ist: die Container, die
`amepkg` erstellt hat, die Apps in Ihrer Anwendungsübersicht, die daraus stammen, und was aus dem AUR
gebaut wurde, jeweils mit dem Zeitpunkt der letzten Aktualisierung. Der Bericht meldet es, wenn einer zwei
Wochen ohne Aktualisierung geblieben ist. [Container](containers.md).

Wenn etwas Ihre Aufmerksamkeit braucht, übergibt **Den Agenten fragen** es an den
[KI-Agenten](ai-agent.md), der in einem Terminal nachforscht und nichts ändert, bis Sie zustimmen.

## Von Anfang an eingeschaltet

- **Aktualisierungen müssen signiert sein.** Nur mit dem Schlüssel von Amethystora signierte Abbilder
  werden installiert, sodass ein manipuliertes oder untergeschobenes Abbild abgelehnt wird.
  [Aktualisierungen](streams.md#signed-updates).
- **Die Firewall lehnt eingehende Verbindungen ab.** Hereingelassen werden die Erkennung von Druckern und
  Geräten, Windows-Dateifreigaben, die IPv6-Einrichtung und GSConnect; Ihrem Tailscale-Tailnet wird
  vertraut. Um etwas anderes hereinzulassen, etwa einen Entwicklungsserver, den Sie von Ihrem Telefon aus
  erreichen möchten, verwenden Sie die App **Firewall** oder `sudo firewall-cmd --add-port=8080/tcp`
  (fügen Sie `--permanent` hinzu, damit es einen Neustart überdauert).
- **Zehn falsche Passwörter in Folge** sperren ein Konto für zehn Minuten. Entsperren Sie es vorher von
  einem anderen Administratorkonto aus mit `sudo faillock --user <name> --reset`.
- **Wiederholt fehlgeschlagene Anmeldungen über das Netzwerk** führen zur Sperrung der Adresse: Fünf
  Fehlschläge innerhalb von zehn Minuten kosten eine Stunde, und jede weitere Sperre derselben Adresse
  dauert länger, bis zu einem Tag. Überwacht wird der SSH-Server, das Einzige hier, bei dem man sich über
  das Netzwerk anmelden kann, und das Tailnet bleibt unberührt. `ame security addresses` listet auf, was
  gesperrt ist, und `ame security addresses <address>` lässt eine Adresse wieder herein.
- **Der SSH-Server ist ausgeschaltet** und lehnt Root-Anmeldungen ab, wenn Sie ihn einschalten.
- **Anwendungen können die Tastatureingaben der anderen nicht belauschen.** Flatpak-Apps werden X11 und
  Eingabegeräte verweigert ([Software installieren](software.md#what-flatpak-apps-may-not-do)), und die
  Tastenumbelegung, die jeden Tastendruck liest, bleibt ausgeschaltet, bis Sie sie anfordern.
- **Der Kernel ist gehärtet:** Speicher wird bei der Vergabe gelöscht, Kernel-Adressen sind verborgen,
  Programme können den Speicher der anderen nicht lesen, und selten genutzte Module (alte
  Netzwerkprotokolle und Dateisysteme, FireWire) können nicht geladen werden. `gdb -p` für einen Prozess,
  den Sie nicht gestartet haben, erfordert `sudo`.
- **Der Kernel lässt sich nicht umschreiben, während er läuft.** Lockdown blockiert das Laden nicht
  vertrauenswürdiger Module, das Schreiben nach `/dev/mem` und das Ausspähen von Kernel-Speicher mit BPF,
  sodass eine Kompromittierung von root mit dem nächsten Neustart endet. Er braucht eingeschaltetes Secure
  Boot und den registrierten Schlüssel ([Hardware](hardware.md#secure-boot)). Auf einem Rechner, auf dem
  sich Secure Boot nicht einschalten lässt, entfernen Sie ihn mit
  `sudo rpm-ostree kargs --delete=lockdown=integrity`. Die NVIDIA-Abbilder verwenden ihn nicht.
- **Was einen Neustart überdauern würde, wird überwacht, und Sie werden informiert.** Das Audit-Protokoll
  zeichnet Änderungen an Konten auf, daran, wer `sudo` verwenden darf, an dem, was von selbst startet (Ihre
  Autostart-Einträge, Ihre eigenen systemd-Dienste, die Startdateien der Shell), und an den
  Sicherheitseinstellungen, sowie jedes Programm, das ein Eingabegerät öffnet. Der Sicherheitswächter liest
  es alle 15 Minuten und sagt, was sich geändert hat und welches Programm es geändert hat.
  [Der Sicherheitswächter](security-watcher.md).
- **Ein Virenscan läuft jede Woche** über Ihre persönlichen und temporären Ordner, und **Lynis prüft die
  Einstellungen jeden Monat.** Standardmäßig löscht oder verschiebt keines von beiden etwas: Ein
  Fehlalarm, der Ihnen eine gewünschte Datei wegnimmt, ist schlimmer als das meiste, was er entfernen
  würde. `ame security scan` zeigt, was der letzte Scan gefunden hat, und `ame security scan now` startet
  einen. [Nach Viren suchen](viruses.md#scanning-for-viruses) erledigt dasselbe in einem Fenster, und in den
  [Einstellungen](security-settings.md) lassen Sie Scans ihre Funde in Quarantäne verschieben oder löschen.

## Einschalten lohnt sich
