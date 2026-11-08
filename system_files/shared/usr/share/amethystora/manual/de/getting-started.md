# Erste Schritte

Ein paar Minuten jetzt, und der Rechner kümmert sich von da an selbst um sich. Keiner dieser Schritte
ist Pflicht, aber jeder lohnt sich, einmal erledigt zu werden.

## 1. Secure Boot dem Abbild vertrauen lassen

Wenn Secure Boot eingeschaltet ist, laden die im Abbild enthaltenen Kernel-Module (DisplayLink-Docks,
virtuelle Kameras) erst, wenn ihr Signaturschlüssel registriert ist. Vielleicht wurden Sie beim ersten
Start schon danach gefragt; falls nicht, führen Sie aus:

```bash
ame security secure-boot
```

Beim nächsten Neustart erscheint ein blauer Bildschirm, der MOK-Manager. Wählen Sie **Enroll MOK**,
dann **Continue**, dann **Yes**, und geben Sie das Passwort `amethystora` ein. Der Bildschirm verwendet
eine US-Tastaturbelegung (QWERTY): Auf einer deutschen Tastatur tippen Sie das y mit der Z-Taste.
[Hardware](hardware.md#secure-boot) enthält die Einzelheiten.

## 2. Prüfen, ob alles eingeschaltet ist

```bash
ame security status
```

Ein Bildschirm voll mit dem, was die Sicherheitseinstellungen des Rechners tatsächlich tun, und was bei
allem zu tun ist, das nicht stimmt. Der Befehl ändert nichts und fragt nach keinem Passwort. Derselbe
Bericht steht in der Anwendungsübersicht als **Sicherheit**, das auch nach Viren sucht
([Sicherheit](viruses.md#scanning-for-viruses)).

## 3. Ihre Apps installieren

Die Apps, die Amethystora mitbringt (Taschenrechner, Texteditor, Erweiterungsverwaltung und der Rest),
installieren sich von selbst, sobald der Rechner zum ersten Mal online ist, und erscheinen in der
Anwendungsübersicht, sobald jede fertig ist. Um sie sofort zu haben, führen Sie `ame apps flatpaks` aus.
Deinstallieren Sie alle, die Sie nicht wollen; sie bleiben deinstalliert.

Öffnen Sie dann **Bazaar**, das Software-Center, und installieren Sie von Flathub, was Sie nutzen. Der
Browser ist Firefox; wenn Sie lieber Chrome oder Brave hätten, finden Sie diese ebenfalls in Bazaar.
[Software installieren](software.md) behandelt Befehlszeilenwerkzeuge und alles andere.

Sie kommen von einem anderen Amethystora-Rechner, der gesichert wurde? `ame restore-setup` stellt dessen
Apps, Container, Erweiterungen und Thema aus der Sicherung wieder her ([Sicherungen](backups.md)).

## 4. Machen Sie es sich zu eigen

- `Super+Ctrl+Shift+Space` wählt ein Thema. Es färbt den ganzen Desktop, das Terminal und den Prompt auf
  einmal neu. [Themen](themes.md).
- Fenster ordnen sich von selbst als Kacheln an, nebeneinander auf einem Streifen, der scrollt. Öffnen
  Sie zwei oder drei Apps und wechseln Sie mit `Super+Left` und `Super+Right` zwischen ihnen.
  [Fenster kacheln](tiling.md). Wenn Sie lieber Fenster hätten, die frei schweben und
  sich überlappen, wechselt `ame desktop layout classic` zu ihnen
  ([Klassische Fenster](tiling.md#classic-windows)).
- `Super+Alt+Space` öffnet das Amethystora-Menü, das alles in diesem Handbuch mit der Tastatur erreicht.

## 5. Eine Sicherung einrichten

```bash
ame backup
```

Eine tägliche Sicherung an einen Ort, an dem Ransomware auf diesem Rechner nichts löschen kann. Der
Befehl führt Sie durch die Wahl des Ortes. [Sicherheit](backups.md).

## 6. Die Tasten lernen

Die Seite [Tastenkürzel](../keybindings.md) ist lesenswert. Lassen Sie sie in Ihrer ersten Woche auf
einer zweiten Arbeitsfläche geöffnet.

## Dann vergessen Sie es einfach

Aktualisierungen kommen von selbst und werden beim nächsten Neustart angewendet. Fahren Sie den Rechner
herunter, wenn Sie für heute fertig sind, und er bleibt aktuell, ohne dass Sie daran denken müssen.
[Aktualisierungen](updates.md) erklärt, was dabei geschieht und wie Sie eine rückgängig machen.
