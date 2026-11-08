# Volver a la versión anterior

El sistema anterior siempre se conserva. Si una actualización trae un problema:

- **En el menú de arranque,** elija la segunda entrada, que es el sistema anterior a la actualización.
  No se cambia nada, y el siguiente reinicio vuelve al más reciente. Al iniciar sesión, una
  notificación indica que está usando la versión anterior; al pulsarla se abre Actualizaciones del
  sistema, que le ofrece **Conservar esta versión**, que la convierte en la que arranca el equipo y
  pide su contraseña, o **Reiniciar en la más reciente**.
- **Para quedarse en el sistema anterior** desde una terminal, conviértalo en el predeterminado y
  reinicie:

```bash
sudo bootc rollback
systemctl reboot
```

Sus archivos y su configuración son los mismos en ambos. La próxima actualización, automática o no,
trae de vuelta la versión más reciente, así que, para quedarse donde está hasta que salga una
solución, desactive las actualizaciones automáticas en Actualizaciones del sistema, o
[quédese en la compilación](#holding-on-to-one-build) que está usando. Después
[informe del problema](troubleshooting.md#reporting-a-problem), y vuelva a seguir el canal cuando
salga una imagen corregida.

## Quedarse en una compilación

Cada compilación tiene además una etiqueta propia, como `stable-44.20260922`, a partir de la versión
que figura en las notas de la versión. Cambiar a ella fija el equipo en esa compilación, lo que
resulta útil mientras espera una solución. Un equipo fijado en una sola compilación no recibe
actualizaciones; `ame security status` se lo recuerda, e imprime el comando para volver a seguir el
canal.
