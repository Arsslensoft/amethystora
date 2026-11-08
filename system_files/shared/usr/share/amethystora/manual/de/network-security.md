# Im Netzwerk

## Browserschutz

```bash
ame security browser
```

Eine Erweiterung sieht jede Seite, die Sie öffnen, und so kommen die meisten Passwortdiebe herein. Mit
eingeschaltetem Browserschutz installieren Browser nur die Erweiterungen auf einer Liste: die
Passwortmanager Bitwarden, 1Password, Proton Pass und KeePassXC sowie die, die Sie mit
`ame security browser allow <id>` hinzufügen. Webseiten werden außerdem von USB-, seriellen, HID- und
Bluetooth-Geräten ferngehalten. Er gilt für Firefox, Brave, Chrome, Chromium und Edge, ob jetzt oder
später installiert, für jedes Konto auf dem Rechner. Andere Browser sind nicht abgedeckt.

Erlauben Sie die Erweiterungen, die Sie verwenden, bevor Sie ihn einschalten. Firefox entfernt beim
nächsten Start die, die nicht auf der Liste stehen, samt ihren Einstellungen; die anderen Browser schalten
ihre aus. Firefox zeigt die ID jeder Erweiterung in `about:support` unter Add-ons, die anderen auf ihrer
Erweiterungsseite bei eingeschaltetem Entwicklermodus. Themes, Wörterbücher und Sprachpakete in Firefox
bleiben unberührt.

Was Sie erlaubt haben, steht in `/etc/amethystora/browser-extensions`, eine ID pro Zeile. Um eine
zurückzunehmen, entfernen Sie ihre Zeile und führen Sie `ame security browser on` erneut aus. Er ist
standardmäßig ausgeschaltet: Firefox kommt genau so, wie Fedora es baut, und was es installieren darf,
entscheiden Sie, nicht das Abbild.

## Netzwerkschutz

```bash
ame security settings network
```

Suricata prüft jede Verbindung, die dieser Rechner aufbaut und annimmt, anhand der Regeln von Emerging
Threats Open, die es täglich herunterlädt:

- **`watch`** meldet, was es erkennt, und blockiert nichts. Beginnen Sie hier: Eine Woche davon zeigt,
  wobei das Blockieren im Weg wäre.
- **`block`** trennt außerdem Verbindungen zu bekannter Schadsoftware und ihren Steuerservern, zu Exploits,
  Phishing-Seiten und Kryptominern sowie zu Adressen auf den Sperrlisten von Spamhaus und DShield. Was die
  Regeln mit weniger Sicherheit erkennen, wird nur gemeldet.

Jede blockierte Verbindung erscheint als Benachrichtigung. **Netzwerk** in der App **Sicherheit** listet
auf, was blockiert und bemerkt wurde, ebenso `ame security connections`. Wenn eine Verbindung von Ihnen
stammte und gewollt war, lässt **Erlauben** oder `ame security connections allow <rule>` genau diese eine
Regel auf diesem Rechner aus, und `ame security connections block <rule>` nimmt sie wieder auf.

Was auch immer damit schiefgeht, Ihre Verbindungen funktionieren weiter: Während Suricata startet, neu
startet, nicht hinterherkommt oder anhält, passieren sie ungeprüft, statt gar nicht. Eine verschlüsselte
Verbindung wird bis zu ihrem Handshake geprüft, dem Servernamen und dem Zertifikat, weil sich das Folgende
nicht lesen lässt. Das Protokoll von Suricata behält, was es erkannt hat, und sonst nichts, nie eine Liste
der Websites, die Sie besuchen. Es prüft die eigenen Verbindungen dieses Rechners, nicht, was er für
virtuelle Maschinen oder als root laufende Container weiterleitet, und belegt einige hundert Megabyte
Arbeitsspeicher. Der Netzwerkschutz ist standardmäßig ausgeschaltet: Etwas zu blockieren ist eine
Entscheidung über Ihren eigenen Datenverkehr, und die treffen Sie.
