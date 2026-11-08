# Anmeldungen

## Sicherheitsschlüssel

Ein FIDO2-Sicherheitsschlüssel (YubiKey, Thetis oder ein Modell mit Fingerabdruck wie der YubiKey Bio)
kann Sie anmelden, den Bildschirm entsperren und `sudo`- sowie Administratorabfragen bestätigen, statt
Ihres Passworts:

```bash
ame security key
```

Wählen Sie **Add a fingerprint key** für ein Modell mit Fingerabdruck, damit der Schlüssel Ihren
Fingerabdruck prüft und nicht nur eine Berührung. Der Schlüssel braucht zuerst eine PIN, und ein Modell mit
Fingerabdruck einen registrierten Finger: Legen Sie beides in Firefox unter `about:webauthn` fest oder auf
einem YubiKey Bio mit `ykman fido fingerprints add`.
Registrieren Sie einen zweiten Schlüssel als Ersatz. Ihr Passwort funktioniert weiterhin, sobald kein
registrierter Schlüssel eingesteckt ist.

## Anmeldung per Fingerabdruck

Auf einem Rechner mit Fingerabdruckleser kann ein Finger Sie anmelden, den Bildschirm entsperren und
`sudo`- sowie Administratorabfragen bestätigen, und Ihr Passwort funktioniert daneben weiterhin:

```bash
ame security fingerprint
```

**Add a finger** registriert einen; registrieren Sie einen zweiten als Ersatz.
**Einstellungen › System › Benutzer** erledigt dasselbe unter **Fingerabdruck-Anmeldung**. Zwei Dinge
fragen weiterhin nach dem Passwort: die Passphrase der Festplatte beim Start und der Schlüsselbund, den
eine Anmeldung per Finger gesperrt lässt, bis eine App ihn braucht. Die Lesegeräte, die funktionieren,
sind unter [fprint.freedesktop.org](https://fprint.freedesktop.org/supported-devices.html) aufgeführt.

## Jugendschutz

Machen Sie das Konto eines Kindes unter **Einstellungen › System › Benutzer** zu einem Standardkonto,
nicht zu einem Administrator, und öffnen Sie dann **Jugendschutz** in der Anwendungsübersicht oder auf der
Seite dieses Kontos in den Einstellungen. Dort legen Sie fest, welche installierten Apps das Konto öffnen
darf, ob es Apps installieren darf und bis zu welcher Altersfreigabe, und ab GNOME 50, wie lange am Tag es
den Rechner benutzen darf und ab welcher Uhrzeit am Abend nicht mehr. Wenn die Zeit abgelaufen ist, wird
der Bildschirm gesperrt.
