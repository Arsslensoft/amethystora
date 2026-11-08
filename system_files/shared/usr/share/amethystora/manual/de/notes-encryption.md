# Ihre Notizen schützen

## Verschlüsselung

Bei eingeschalteter Verschlüsselung werden Ihre Notizen, Aufgaben und Anhänge mit AES-256-GCM
verschlüsselt auf die Festplatte geschrieben, mit einem zufälligen Schlüssel, den nur Ihre Passphrase
oder ein [Passkey](#passkeys), den Sie hinzufügen, öffnen kann. Die Passphrase wird mit scrypt gestreckt,
was für jeden Rateversuch 128 MiB Speicher kostet, sodass das Durchprobieren von Passphrasen langsam ist.

Sie bleibt auch gegen Quantencomputer sicher. Was ein Quantencomputer bricht, ist
Public-Key-Kryptografie (RSA, elliptische Kurven), und davon wird hier keine verwendet. Gegen eine
Chiffre wie AES kann ein Quantencomputer bestenfalls die Schlüssellänge halbieren. Damit behält AES-256
eine Stärke von 128 Bit, weshalb NIST und die CNSA 2.0 der NSA AES-256 für die Zeit nach dem Aufkommen
von Quantencomputern beibehalten.

> **Niemand kann eine vergessene Passphrase wiederherstellen.** Nicht Amethystora, kein Administrator,
> niemand. Bewahren Sie sie an einem sicheren Ort auf, und halten Sie eine [Sicherung](#backing-up) bereit.

Ihre Notizen werden gesperrt:

- nach 10 Minuten ohne Nutzung von Notizen (**Einstellungen**, **Bei Abwesenheit sperren**),
- wenn der Bildschirm gesperrt wird, der Rechner in Bereitschaft geht oder das Fenster geschlossen wird,
- wenn Sie **Ctrl+L** drücken oder **Sperren** in der Seitenleiste wählen.

In den **Einstellungen** ändern Sie auch die Passphrase und schalten die Verschlüsselung aus oder wieder
ein.

### Passkeys

Ein FIDO2-Sicherheitsschlüssel (YubiKey, Thetis oder ein Modell mit Fingerabdruck wie der YubiKey Bio)
kann Ihre Notizen öffnen, sodass Sie die Passphrase nicht jedes Mal eingeben müssen. Die Passphrase
funktioniert weiterhin, und sie öffnet Ihre Notizen, wenn der Schlüssel verloren geht: Ein Passkey ist ein
zweiter Zugang, nie der einzige.

Um einen hinzuzufügen, stecken Sie den Schlüssel ein, öffnen die **Einstellungen** und wählen **Passkey
hinzufügen**. Geben Sie Ihre Passphrase und die PIN des Schlüssels ein, oder lassen Sie die PIN bei einem
Schlüssel, der Ihren Fingerabdruck liest, leer. Der Schlüssel blinkt zweimal; berühren Sie ihn jedes Mal.
Danach bietet der Sperrbildschirm **Mit Passkey öffnen** an, was nach der PIN des Schlüssels oder Ihrem
Fingerabdruck und einer Berührung fragt.

- **Der Schlüssel braucht eine PIN**, und ein Modell mit Fingerabdruck einen registrierten Finger. Legen
  Sie beides in Firefox unter `about:webauthn` fest. Eine Berührung allein öffnet Ihre Notizen nie: Wer
  den Schlüssel findet, kann ihn ohne die PIN oder Ihren Finger nicht benutzen, und der Schlüssel sperrt
  seine PIN nach acht Fehlversuchen.
- **Fügen Sie einen zweiten Schlüssel als Ersatz hinzu**, einen nach dem anderen. Jeder wird in den
  **Einstellungen** aufgeführt, wo **Entfernen** ihm das Öffnen Ihrer Notizen wieder entzieht.
- **Wenn ein Schlüssel verloren geht**, entfernen Sie ihn. Wer den Schlüssel besitzt, seine PIN kennt und
  eine ältere Kopie Ihres Notizenordners hat, könnte diese Kopie noch öffnen. Wenn Sie die
  Verschlüsselung aus- und wieder einschalten, bekommen Ihre Notizen einen neuen Schlüssel, den kein
  entfernter Passkey öffnet; fügen Sie danach die Schlüssel, die Sie noch haben, erneut hinzu.
- Eine [Sicherung](#backing-up) fragt immer nach der Passphrase: Passkeys sind nicht Teil davon.

Der Schlüssel speichert nichts von Ihren Notizen. Er enthält ein Geheimnis, das ihn nie verlässt, und
berechnet nach geprüfter PIN oder geprüftem Fingerabdruck einen Wert (HMAC-SHA256, das `hmac-secret` von
FIDO2), mit dem eine zweite Kopie des Schlüssels Ihrer Notizen verschlüsselt ist. Was auf der Festplatte
liegt, ist weiterhin allein mit AES-256 verschlüsselt.

## Sichern

Da nichts synchronisiert wird, bewahren Sie eine Kopie woanders auf. In den **Einstellungen**:

- **In eine Datei sichern** schreibt eine `.amnotes`-Datei mit allem darin, Anhänge eingeschlossen,
  verschlüsselt wie Ihre Notizen. Sie lässt sich mit der Passphrase, die Sie beim Erstellen hatten, auf
  jedem Rechner öffnen: Wählen Sie sie unter **Importieren** aus.
- **Als Markdown exportieren** schreibt jede Notiz als Markdown-Datei in einen Ordner pro Notizbuch und
  jedes Projekt als Checkliste. Diese Dateien sind **nicht** verschlüsselt.

Ihre Notizen selbst liegen in `~/.local/share/amethystora-notes`.
