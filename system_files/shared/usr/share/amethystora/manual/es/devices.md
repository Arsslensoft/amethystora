# Dispositivos

## Impresoras y escáneres

La mayoría de las impresoras se encuentran en la red y funcionan sin más, sin controlador. Para las
demás se incluyen controladores para impresoras HP y para muchas impresoras Brother y láser antiguas.
Añada una impresora en **Configuración → Impresoras**.

## Teclados, ratones y llaves de seguridad

- **Ratones para juegos:** el servicio que los configura está incluido; instale **Piper** desde Bazaar
  para cambiar los botones, la iluminación y la resolución.
- **Reasignar teclas:** `ame security input-remapper` activa Input Remapper ([Seguridad](devices-security.md#key-remapping)
  explica por qué empieza desactivado).
- Las **llaves de seguridad FIDO2** pueden sustituir a su contraseña: [Seguridad](logins.md#security-keys).

## Cámaras

Las cámaras virtuales (la de OBS, por ejemplo) funcionan sin instalar nada, gracias al módulo loopback
incluido en la imagen.
