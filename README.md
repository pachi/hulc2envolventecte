# hulc2envolventecte

Exportación de datos de HULC a EnvolventeCTE

Esta aplicación permite exportar los datos de un proyecto de la `Herramienta unificada LIDER-CALENER (HULC)` al formato JSON que utiliza la aplicación web para el cálculo de parámetros energéticos de la envolvente térmica [`EnvolventeCTE`](https://pachi.github.io/envolventecte).

## Uso

Esta aplicación se utiliza desde la línea de comandos, y debe inidicar como parámetro el directorio del proyecto de HULC que desea exportar, redirigiendo la salida a un archivo para su posterior importación desde la interfaz web de EnvolventeCTE:

```
    $ hulc2envolventecte datos/proyecto/hulc > salida.json
```

## Licencia

Esta aplicación es software libre y se distribuye bajo una licencia MIT. Consulte el archivo LICENSE para el texto completo.

El código está disponible en http://github.com/pachi/hulc2envolventecte

## Autores

Copyright (c) 2018 Rafael Villar Burke <pachi@ietcc.csic.es>,  Daniel Jiménez González <danielj@ietcc.csic.es>, Marta Sorribes Gil <msorribes@ietcc.csic.es>
