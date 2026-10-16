# Changelog

## Sin publicar

## 1.0.8 — 2026-10-16

### Añadido
- Instalación automática de ClamAV en Windows (winget) cuando no está presente.
- Configuración de freshclam y base de firmas propias por usuario, sin permisos de administrador.
- Actualización automática de firmas al iniciar y cada 4 horas.
- Inicio con el sistema (oculto en la bandeja) e instancia única.
- Idioma español.
- Nueva identidad visual e interfaz minimalista.
- La protección en tiempo real usa `clamd`, que mantiene las firmas cargadas: cada archivo se revisa en milisegundos en vez de cargar la base completa por archivo.

### Corregido
- Una actualización fallida ya no se muestra como "actualizado".
- Se evitan dos actualizaciones de firmas simultáneas.
- Los ajustes booleanos ya se pueden desactivar.
- ClamAV se detecta en `C:\Program Files\ClamAV` aunque no esté en el PATH.
- Un escaneo ya no se pierde al cambiar de sección: el progreso y los resultados se mantienen, y al volver a "Escanear" se muestra el escaneo en curso.
- El escaneo empieza de inmediato; el recuento de archivos para la barra de progreso se hace en paralelo.
- El escaneo principal ya no revisa dos veces las carpetas que están dentro de la carpeta de usuario.
- Las exclusiones se respetan en el recuento de archivos y en los escaneos programados.
- Tiempo real: reiniciarlo o cambiar de modo ya no deja vigilancia duplicada, no se bloquea durante un escaneo, espera a que el archivo termine de escribirse y respeta "reescanear al modificar" según el modo.
- Si la protección en tiempo real no puede activarse, la app lo indica en vez de mostrarla como activa.
- Borrar el historial de las últimas 24 h, 7 o 30 días ya no elimina el historial antiguo.
- El historial ya no pierde entradas cuando varias acciones escriben a la vez.
- Las exclusiones funcionan en Windows, y la carpeta de cuarentena ya no se escanea.
- Las amenazas en rutas con espacios ya se pueden poner en cuarentena o borrar desde los resultados.
- Un escaneo detenido se registra como cancelado, no como "amenazas encontradas".
- Los escaneos programados se crean aunque la app no esté en el PATH, se ejecutan también con batería y muestran su última ejecución en Windows en español.
- La ventana ya no se congela durante las acciones de cuarentena, el programador o las estadísticas.
- El desinstalador quita el inicio automático y las tareas programadas.
- Seguridad: los identificadores de registros y de cuarentena se validan, y se quitó un permiso que dejaba abrir cualquier archivo.

Detalle completo en [CAMBIOS.md](CAMBIOS.md).

### Eliminado
- Actualizador automático de la aplicación: las nuevas versiones se distribuyen como instalador. Las firmas de virus siguen actualizándose solas.
