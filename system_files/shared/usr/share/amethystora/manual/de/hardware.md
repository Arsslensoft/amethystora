# Hardware

Amethystora läuft am besten auf Hardware, die Linux gut unterstützt: Grafik von Intel und AMD und die
Laptops, die ihre Hersteller für Linux zertifizieren. Das meiste funktioniert, sobald es eingesteckt ist;
diese Seite handelt von dem, was einen Schritt von Ihnen braucht.

## Secure Boot

Lassen Sie Secure Boot eingeschaltet. Damit startet der Rechner nur einen Kernel, den er geprüft hat, und
der [Lockdown](security.md) des Kernels hat eine Grundlage.

Die im Abbild enthaltenen Kernel-Module für DisplayLink-Docks und virtuelle Kameras sind mit dem eigenen
Schlüssel von Amethystora signiert, und der Firmware muss einmal gesagt werden, dass sie ihm vertrauen
soll:

```bash
ame security secure-boot
```

Starten Sie neu, und ein blauer Bildschirm erscheint, bevor das System startet: der MOK-Manager. Wählen
Sie **Enroll MOK**, dann **Continue**, dann **Yes**, und geben Sie das Passwort `amethystora` ein. Der
Bildschirm verwendet eine US-Tastaturbelegung (QWERTY), unabhängig von Ihrer eigenen, was wichtig ist, wenn
Ihre Tastatur die Buchstaben anders anordnet: Auf einer deutschen Tastatur tippen Sie das y mit der
Z-Taste. `ame security status` sagt Ihnen, ob die Schlüssel registriert sind.

Wenn Secure Boot ausgeschaltet ist, weist einmalig eine Benachrichtigung darauf hin. Schalten Sie es in den
Firmware-Einstellungen des Rechners ein und führen Sie dann den obigen Befehl aus.

## Grafik

- **Intel und AMD** funktionieren sofort. Das Entwickler-Abbild fügt ROCm für GPU-Berechnungen auf AMD
  hinzu.
- **NVIDIA** braucht die `-nvidia-open`-Abbilder, die NVIDIAs offenen Kernel-Treiber mitbringen ([Installation](choosing.md#pick-an-image)).
  Der NVIDIA-Treiber wird für den Rechner gebaut und mit dessen eigenem Schlüssel signiert, deshalb
  verwenden die NVIDIA-Abbilder keinen Kernel-Lockdown.
- **Das Abbild ohne NVIDIAs Treiber auf einem Rechner mit NVIDIA-Karte installiert?** Für die
  NVIDIA-Abbilder gibt es kein Installationsprogramm, also starten die meisten NVIDIA-Rechner so. Ein paar
  Minuten nach dem ersten Start bietet eine Benachrichtigung **Switch images…** an, was
  `ame system rebase` mit dem passenden `-nvidia-open`-Abbild öffnet; es nimmt nur signierte Abbilder an
  und fragt vor dem Wechsel nach. Ein NVIDIA-Abbild auf einem Rechner ohne NVIDIA-Karte bietet das Abbild
  ohne den Treiber an, das den Lockdown eingeschaltet lässt. Nichts wechselt von selbst, der Hinweis kommt
  einmal pro Rechner, und **Don't ask again** stellt ihn für immer ab.
