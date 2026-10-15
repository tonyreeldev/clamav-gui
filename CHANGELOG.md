# Changelog

## Sin publicar

### Añadido
- Instalación automática de ClamAV en Windows (winget) cuando no está presente.
- Configuración de freshclam y base de firmas propias por usuario, sin permisos de administrador.
- Actualización automática de firmas al iniciar y cada 4 horas.
- Inicio con el sistema (oculto en la bandeja) e instancia única.
- Idioma español.
- Nueva identidad visual e interfaz minimalista.

### Corregido
- Una actualización fallida ya no se muestra como "actualizado".
- Se evitan dos actualizaciones de firmas simultáneas.
- Los ajustes booleanos ya se pueden desactivar.
- ClamAV se detecta en `C:\Program Files\ClamAV` aunque no esté en el PATH.

### Eliminado
- Actualizador automático de la aplicación: las nuevas versiones se distribuyen como instalador. Las firmas de virus siguen actualizándose solas.
