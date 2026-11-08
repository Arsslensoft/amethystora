# Der Sicherheitswächter

Alle 15 Minuten, oder bei eingeschalteter [Echtzeitüberwachung](security-settings.md) sobald etwas geschieht, sieht
sich der Wächter Folgendes an:

- **Das Audit-Protokoll**: Konten, `sudo`-Regeln, was von selbst startet, die Sicherheitseinstellungen und
  Kernel-Module, die aus einer Anmeldesitzung heraus geladen wurden, jeweils mit dem Programm, das die
  Änderung vorgenommen hat. Ein Programm, das die Tastatur direkt öffnet, wird gemeldet, wenn dieses
  Programm es zum ersten Mal tut; Spiele und Tastenumbelegungen tun das ebenfalls.
- **Das Journal**: ein Konto, das nach falschen Passwörtern gesperrt wurde, drei oder mehr falsche
  Passwörter für `sudo` oder als Administrator, und USB-Geräte, die der
  [USB-Schutz](devices-security.md#block-usb-devices-you-did-not-plug-in) blockiert hat.
- **`/etc` im Vergleich zum Abbild**: Das Abbild behält seine eigene Kopie von `/etc` in `/usr/etc`, sodass
  eine geänderte Sicherheitseinstellung oder eine Datei in `/etc`, die eine des Abbilds in `/usr/lib`
  ersetzt, im Bericht unter **Einstellungen des Abbilds** erscheint.
- **Was es auf diesem System nicht geben sollte**: ein setuid-Programm außerhalb von `/usr`,
  `/etc/ld.so.preload`, eine gewöhnliche Datei in `/dev`, ein Kernel-Modul, das nicht mit dem Abbild kam
  oder nicht signiert ist, und eine Netzwerkschnittstelle, die jedes Paket liest. Das sind die
  **Rootkit-Prüfungen** im Bericht.
- **Was Verbindungen annimmt**: jedes Programm, das im Netzwerk lauscht und das jedes Gerät in Ihrem
  Tailnet erreichen kann. Ein neues wird einmal gemeldet.
- **Was Ihre Sitzung zuerst findet**: ein Programm in `~/.local/bin`, `~/bin` oder dem Ordner von
  Homebrew, das wie einer der Befehle des Systems heißt und das Ihre Shell anstelle des echten ausführt.
  Ein gefälschtes `sudo` oder `ssh` dort ist eine alte Methode, um ein Passwort abzufangen, deshalb werden
  diese am deutlichsten gemeldet; der Ordner von Homebrew zählt nur für diese. Und ein Starter in Ihrer
  Anwendungsübersicht, den kein Container-Export angelegt hat. Jedes wird gemeldet, wenn es auftaucht, und
  erneut, wenn es sich ändert.
- **Netzwerkschutz**, solange er eingeschaltet ist: jede Verbindung, die er blockiert hat, mit dem Weg, sie
  wieder zu erlauben, und was er ohne Blockieren erkannt hat, wenn die Regel es als ernst einstuft.
  [Netzwerkschutz](network-security.md#network-protection).
- **Der Bericht selbst**: wenn eine Prüfung, die meldet, ob etwas ausgeschaltet wurde, etwa die Firewall,
  signierte Aktualisierungen oder das Audit-Protokoll, nicht mehr in Ordnung ist.

Alles Neue erscheint als Benachrichtigung und wartet im Bericht, bis Sie es lesen:

```bash
ame security events
```

`ame security events now` sieht zuerst nach. Wenn Sie die Änderung vorgenommen oder etwas installiert
haben, das sie vorgenommen hat, gibt es nichts zu tun.
