# Solución de problemas

La mayoría de los problemas en un sistema basado en imágenes tienen las mismas tres respuestas:
averiguar qué ha pasado, volver atrás si lo causó una actualización e informar de ello para que la
próxima imagen lo corrija.

## Deje que el agente mire primero

**Diagnose a problem**, en el menú de Amethystora (`Super+Alt+Space`), o desde una terminal:

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

Lee los registros y los informes de fallos, no cambia nada y vuelve con la causa y una solución que
considerar. [El agente de IA](ai-agent.md#diagnose-a-problem) tiene más información.

## Una actualización estropeó algo

Elija el sistema anterior en el menú de arranque para confirmar que fue la actualización.
Actualizaciones del sistema le ofrecerá entonces conservarlo, lo que también hace
`sudo bootc rollback`, hasta que salga una imagen corregida. [Actualizaciones](rolling-back.md).

## Informar de un problema

Informe de los errores en [GitHub](https://github.com/arsslensoft/amethystora/issues), y haga sus
preguntas en las [discusiones](https://github.com/arsslensoft/amethystora/discussions). `ame report`
reúne lo que necesita un informe en un archivo de su carpeta personal (la imagen, sus grupos, los
servicios que fallaron y los errores registrados desde este arranque), se lo muestra todo y después,
si usted lo indica, abre una incidencia nueva con todo rellenado salvo los errores. No se publica nada
hasta que lo envíe; arrastre el archivo si los errores ayudan. Si no, incluya:

- la salida de `rpm-ostree status`, que indica la imagen exacta,
- lo que hizo, lo que esperaba y lo que ocurrió en su lugar,
- las líneas pertinentes de los registros anteriores,
- si la imagen anterior tenía el mismo problema.
