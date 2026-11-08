# Gesperrte Festplatten und Ordner

## Festplatte mit dem TPM entsperren

```bash
ame security disk-unlock
```

Statt bei jedem Start die Passphrase der Festplatte einzugeben, übergibt das TPM des Rechners den
Schlüssel, aber nur, solange Secure Boot eingeschaltet ist und die Signaturschlüssel dieselben sind, die
bei der Einrichtung registriert waren. Fügen Sie eine PIN hinzu, wenn die Festplatte zusätzlich etwas
brauchen soll, das Sie wissen. Ihre Passphrase funktioniert weiterhin, sodass ein zurückgesetztes TPM Sie
nie aussperrt. Das erfordert eine Festplatte, die bei der Installation verschlüsselt wurde.
