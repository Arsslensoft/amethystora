# Geräte und Ihr Bildschirm

## USB-Geräte blockieren, die Sie nicht eingesteckt haben

```bash
ame security usb
```

Ein USB-Gerät kann sich als Tastatur ausgeben und selbst tippen, und ein Hardware-Keylogger kann zwischen
einer echten Tastatur und dem Rechner sitzen. Der USB-Schutz erlaubt, was bei der Einrichtung angeschlossen
ist, und blockiert alles Neue, bis Sie es mit `ame security usb allow` erlauben. Er ist standardmäßig
ausgeschaltet, weil ein Rechner, der beim ersten Start seine eigene Tastatur blockiert, unbenutzbar wäre.

## Tastenumbelegung

Input Remapper läuft als root und liest jeden Tastendruck in jeder Anwendung. Das braucht das Umbelegen,
und in jeder anderen Hinsicht ist es ein Keylogger, deshalb wird es ausgeschaltet ausgeliefert. Wenn Sie
Tasten oder Maustasten umbelegen, schalten Sie es mit `ame security input-remapper` ein.
