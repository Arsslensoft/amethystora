# Los registros

Abra **Registros** desde la cuadrícula de aplicaciones, o ejecute `amethystora-logs`. Se abre en
**Importante**, los errores de todo el equipo desde que arrancó. La barra lateral reparte el resto
entre su sesión, el sistema, el núcleo y el hardware, la seguridad (inicios de sesión, `sudo`, lo que
detectaron las reglas de auditoría) y **Fallos**, cada uno con un recuento de lo nuevo desde el último
arranque cuando importa.

- **Buscar mensajes** examina lo que dicen los mensajes a medida que escribe. El botón `.*` de al lado
  hace que la búsqueda se tome como una expresión regular.
- **Periodo** vuelve al arranque anterior, a cualquier arranque que aún guarde el diario, a la última
  hora, al último día o a la última semana, o a todo. **Nivel** oculta lo que sea menos grave que lo
  que elija.
- **Aplicaciones y servicios** lista todo lo que ha escrito en el diario. Elija una aplicación para
  leer lo que dijo cada vez que se ejecutó, o un servicio del sistema o de su sesión.
- Pulse en una entrada para leerla entera, con todos los campos que guarda el diario. **Solo …** limita
  la vista a su procedencia, y **Alrededor de esta** muestra todo lo registrado en el minuto anterior y
  en el posterior.
- **Seguir** añade las entradas nuevas a medida que se escriben. **Exportar** guarda lo que se muestra
  en un archivo de texto o JSON, y el botón de la terminal abre la misma vista en `journalctl`. El
  comando de la vista también está al pie de la barra lateral.

Los administradores, las cuentas del grupo `wheel`, lo ven todo. Cualquier otra cuenta solo ve su
propia sesión y sus aplicaciones, y las vistas del sistema lo indican.

Lo mismo en una terminal:

| Comando | Muestra |
| --- | --- |
| `journalctl -b -p warning` | Las advertencias y los errores desde este arranque |
| `journalctl -b -1 -p warning` | Lo mismo del arranque anterior, tras un fallo o un cuelgue |
| `journalctl --user -b` | Su propia sesión: el escritorio, sus aplicaciones |
| `systemctl --failed` | Los servicios que no pudieron iniciarse |
| `coredumpctl list` | Los programas que se cerraron inesperadamente |
| `ame system logs-this-boot` | Todo lo de este arranque |
| `ame system local-overrides` | Los archivos de `/etc` que usted, o algo que ejecutó, ha cambiado |
