# Discos y carpetas bloqueados

## Desbloquear el disco con el TPM

```bash
ame security disk-unlock
```

En lugar de escribir la frase de contraseña del disco en cada arranque, el TPM del equipo entrega la
clave, pero solo mientras Secure Boot esté activado y las claves de firma sean las que estaban
registradas cuando lo configuró. Añada un PIN si quiere que el disco necesite además algo que usted
sabe. Su frase de contraseña sigue funcionando, así que un TPM borrado nunca le deja fuera. Necesita
un disco que se cifró al instalarlo.
