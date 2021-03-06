# hulc2envolventecte

Exportación de datos de HULC a EnvolventeCTE

Esta aplicación permite exportar los datos de un proyecto de la `Herramienta unificada LIDER-CALENER (HULC)` al formato JSON que utiliza la aplicación web para el cálculo de parámetros energéticos de la envolvente térmica [`EnvolventeCTE`](https://pachi.github.io/envolventecte).

Esta versión está preparada para funcionar con las versiones de HULC adaptadas al CTE DB-HE 2019.

## Instalación

En la [página de versiones publicadas del proyecto](http://github.com/pachi/hulc2envolventecte/releases) puede encontrar los archivos necesarios para el funcionamiento del programa, que no necesita instalación.

Los archivos distribuidos permiten el uso de la aplicación en sistemas GNU/Linux y MS-Windows:

- `hulc2envolventecte` - ejecutable para GNU/Linux
- `hulc2envolventecte.exe` - ejecutable para MS-Windows
- `hulc2envolventecte.zip` - código fuente comprimido en formato ZIP
- `hulc2envolventecte.tar.gz` - código fuente comprimido en formato .tar.gz


## Uso

Esta aplicación se utiliza desde la línea de comandos, y debe inidicar como parámetro el directorio del proyecto de HULC que desea exportar, redirigiendo la salida a un archivo para su posterior importación desde la interfaz web de EnvolventeCTE:

```
    $ hulc2envolventecte datos/proyecto/hulc > salida.json
```

En sistemas MS-Windows al ejecutar el programa se lanza una interfaz gráfica simple en la que se puede indicar el directorio de proyecto de HULC sobre el que se quiere trabajar, y en el que se realizará la exportación del archivo `.json` generado.

## Licencia

Esta aplicación es software libre y se distribuye bajo una licencia MIT. Consulte el archivo LICENSE para el texto completo.

El código fuente se encuentra disponible en http://github.com/pachi/hulc2envolventecte

## Autores

Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>,  Daniel Jiménez González <danielj@ietcc.csic.es>, Marta Sorribes Gil <msorribes@ietcc.csic.es>
