# Los dispositivos y su pantalla

## Bloquear los dispositivos USB que no conectó usted

```bash
ame security usb
```

Un dispositivo USB puede hacerse pasar por un teclado y escribir solo, y un keylogger físico puede
colocarse entre un teclado real y el equipo. La protección USB permite lo que está conectado cuando la
configura y bloquea todo lo nuevo hasta que lo permita con `ame security usb allow`. Está desactivada
de forma predeterminada, porque un equipo que bloqueara su propio teclado en el primer arranque sería
inutilizable.

## Reasignación de teclas

Input Remapper se ejecuta como root y lee cada tecla que se escribe en cada aplicación. Eso es lo que
necesita la reasignación, y en todo lo demás equivale a un keylogger, así que viene desactivado. Si
reasigna teclas o botones del ratón, actívelo con `ame security input-remapper`.
