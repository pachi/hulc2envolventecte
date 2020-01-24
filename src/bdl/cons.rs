//! Parser del Building Description Language (BDL) de DOE
//!
//! Composiciones constructivas y de materiales
//!
//! - Material (MATERIAL)
//! - Acristalamiento (GLASS-TYPE)
//! - Marco (NAME-FRAME)
//! - Cerramiento (LAYERS)
//! - Composición de hueco (GAP)
//!
//! Configuran la base de datos de materiales por familias y dentro de ellas, por grupos:
//!
//! BBDD
//!
//! - Opacos
//!      - Materiales y productos (MATERIAL (tipo PROPERTIES o RESISTANCE))
//!      - Cerramientos y particiones (LAYERS)
//! - Semitransparentes
//!      - Vidrios (GLASS-TYPE)
//!      - Marcos (NAME-FRAME)
//!      - Composición de huecos y lucernarios (GAP)
//! - Puentes térmicos (THERMAL-BRIDGE)?

use failure::Error;
use std::collections::HashMap;
use std::convert::TryFrom;

use super::{extract_f32vec, extract_namesvec, BdlBlock};

/// Elementos constructivos y de materiales pertenecientes a la base de datos
/// Se organizan por nombre y grupo (tipo)
#[derive(Debug, Default)]
pub struct DB {
    /// Material o producto
    pub materials: HashMap<String, Material>,
    /// Composición por capas (opacos)
    pub layers: HashMap<String, Layers>,
    /// Composición por capas (huecos)
    pub windows: HashMap<String, Gap>,
    /// Vidrio
    pub glasses: HashMap<String, Glass>,
    /// Marco
    pub frames: HashMap<String, Frame>,
    /// Puente térmico
    pub tbridges: HashMap<String, ThermalBridge>,
}

/// Material definido por sus propiedades térmicas o por resistencia
#[derive(Debug, Clone, Default)]
pub struct Material {
    /// Nombre del material
    pub name: String,
    /// Grupo al que pertenece (biblioteca)
    pub group: String,
    /// Definición detallada de propiedades
    pub properties: Option<MaterialProperties>,
    /// Definición de resistencia térmica
    pub resistance: Option<MaterialResistance>,
}

/// Definición de propiedades termofísicas y grosor
#[derive(Debug, Copy, Clone, Default)]
pub struct MaterialProperties {
    /// Espesor, d (m)
    /// En LIDER antiguo no se define este valor
    pub thickness: Option<f32>,
    /// Conductividad térmica, lambda (W/mK)
    pub conductivity: f32,
    /// Densidad, rho (kg/m3)
    pub density: f32,
    /// Calor específico, C_p (J/kg K)
    pub specificheat: f32,
    /// Factor de difusividad al vapor de agua, mu (-)
    /// En archivos de LIDER antiguo se pone por defecto 0.0 (no definido)
    pub vapourdiffusivity: Option<f32>,
}

/// Definición por resistencia térmica
#[derive(Debug, Copy, Clone, Default)]
pub struct MaterialResistance {
    /// Resistencia térmica R (m2K/W)
    pub resistance: f32,
}

impl TryFrom<BdlBlock> for Material {
    type Error = Error;

    /// Conversión de bloque BDL a material
    ///
    /// Ejemplo en BDL:
    /// ```text
    ///     "FR Entrevigado de EPS moldeado descolgado -Canto 450 mm" = MATERIAL
    ///     TYPE              = PROPERTIES
    ///     THICKNESS         =           0.45
    ///     THICKNESS_CHANGE         = YES
    ///     THICKNESS_MAX         =              2
    ///     THICKNESS_MIN         =          0.001
    ///     CONDUCTIVITY      =      0.4787234
    ///     DENSITY           =           1280
    ///     SPECIFIC-HEAT     =           1000
    ///     VAPOUR-DIFFUSIVITY-FACTOR =             60
    ///     NAME          = "FR Entrevigado de EPS moldeado descolgado -Canto 450 mm"
    ///     GROUP         = "Forjados reticulares"
    ///     IMAGE          = "ladrillo.bmp"
    ///     NAME_CALENER   = "oldeado descolgado -Canto 450 "
    ///     LIBRARY       = NO
    ///     UTIL          =  NO
    ///     OBSOLETE      = NO
    ///     ..
    ///     
    ///     $ LIDER antiguo
    ///     "AislanteREF" = MATERIAL
    ///         TYPE = PROPERTIES
    ///         CONDUCTIVITY = 0.036
    ///         DENSITY = 30
    ///         SPECIFIC-HEAT = 1800
    ///         ..
    /// ```
    /// TODO: Propiedades no convertidas:
    /// TODO: THICKNESS_CHANGE, THICKNESS_MAX, THICKNESS_MIN, IMAGE, NAME_CALENER, LIBRARY, UTIL, OBSOLETE
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        // El LIDER antiguo no guardaba grupos
        let group = attrs
            .remove_str("GROUP")
            .unwrap_or("Materiales".to_string());
        let (properties, resistance) = match attrs.remove_str("TYPE")?.as_ref() {
            "PROPERTIES" => {
                // XXX: En LIDER antiguo no se define este valor
                let thickness = attrs.remove_f32("THICKNESS").ok();
                let conductivity = attrs.remove_f32("CONDUCTIVITY")?;
                let density = attrs.remove_f32("DENSITY")?;
                let specificheat = attrs.remove_f32("SPECIFIC-HEAT")?;
                // XXX: En LIDER antiguo no se define este valor
                let vapourdiffusivity = attrs.remove_f32("VAPOUR-DIFFUSIVITY-FACTOR").ok();
                (
                    Some(MaterialProperties {
                        thickness,
                        conductivity,
                        density,
                        specificheat,
                        vapourdiffusivity,
                    }),
                    None,
                )
            }
            _ => {
                let resistance = attrs.remove_f32("RESISTANCE")?;
                (None, Some(MaterialResistance { resistance }))
            }
        };
        Ok(Self {
            name,
            group,
            properties,
            resistance,
        })
    }
}

/// Definición de capas
#[derive(Debug, Clone, Default)]
pub struct Layers {
    /// Nombre
    pub name: String,
    /// Grupo al que pertenece (biblioteca)
    pub group: String,
    /// Lista de nombres de materiales de las capas
    pub material: Vec<String>,
    /// Lista de espesores de las capas ([m, m, ...])
    pub thickness: Vec<f32>,
}

impl TryFrom<BdlBlock> for Layers {
    type Error = Error;

    /// Conversión de bloque BDL a definición de capas
    ///
    /// Ejemplo:
    /// ```text
    ///     "muro_opaco" = LAYERS
    ///         GROUP        = "envolvente"
    ///         NAME_CALENER = ""
    ///         NAME         = "muro_opaco"
    ///         TYPE-DEFINITION = 1
    ///         MATERIAL     = ("Mortero de cemento o cal para albañilería y para revoco/enlucido 1000 < d < 1250","EPS Poliestireno Expandido [ 0.029 W/[mK]]","1/2 pie LP métrico o catalán 80 mm< G < 100 mm","MW Lana mineral [0.031 W/[mK]]","Placa de yeso laminado [PYL] 750 < d < 900")
    ///         THICKNESS = (          0.015,           0.06,          0.115,           0.04,           0.02)
    ///         LIBRARY       =  NO
    ///         UTIL          =  YES
    ///         IMAGE = ""
    ///         DEFAULT = NO
    ///         ..
    ///
    ///     $ LIDER antiguo
    ///     "CONST_referencia-5" = LAYERS
    ///         MATERIAL = ( "PlaquetaREF","MorteroREF","ForjadoREF" )
    ///         THICKNESS = ( 0.015, 0.020, 0.250 )
    ///         ..
    /// ```
    /// TODO: Propiedades de Layers no convertidas:
    /// TODO: IMAGE, NAME_CALENER, LIBRARY, UTIL, TYPE-DEFINITION, DEFAULT
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        // En LIDER antiguo no se guarda el grupo
        let group = attrs.remove_str("GROUP").unwrap_or("Capas".to_string());
        let material = extract_namesvec(attrs.remove_str("MATERIAL")?);
        let thickness = extract_f32vec(attrs.remove_str("THICKNESS")?)?;
        Ok(Self {
            name,
            group,
            material,
            thickness,
        })
    }
}

/// Definición de hueco o lucernario (GAP)
#[derive(Debug, Clone, Default)]
pub struct Gap {
    /// Nombre
    pub name: String,
    /// Grupo al que pertenece (biblioteca)
    pub group: String,
    /// Acristalamiento (GLASS-TYPE)
    pub glass: String,
    /// Grupo al que pertenece el acristalamiento
    pub glassgroup: String,
    /// Marco (NAME-FRAME)
    pub frame: String,
    /// Grupo al que pertenece el marco
    pub framegroup: String,
    /// Porcentaje de hueco cubierto por el marco (%)
    pub framepct: f32,
    /// Permeabilidad al aire (m3/hm2 a 100Pa)
    pub infcoeff: f32,
    /// Porcentaje de U debido a intercalarios y cajón de persiana (%)
    pub deltau: f32,
    /// Transmitancia total de energía del acristalameinto con los dispositivo de sombra móvil activados (g_gl;sh;wi) (-)
    pub gglshwi: Option<f32>,
}

impl TryFrom<BdlBlock> for Gap {
    type Error = Error;

    /// Conversión de bloque BDL a definición de hueco o lucernario (GAP)
    ///
    /// Ejemplo:
    /// ```text
    ///      "ventana estandar" = GAP
    ///           NAME           = "ventana estandar"
    ///           TYPE           = 1
    ///           GROUP          = "muro_cortina"
    ///           GROUP-GLASS         = "Vidrios"
    ///           GLASS-TYPE          = "Doble baja emisividad argon"
    ///           GROUP-FRAME       = "Metálicos en posición vertical"
    ///           NAME-FRAME        = "VER_Con rotura de puente térmico mayor de 12 mm"
    ///           PORCENTAGE        = 20.000000
    ///           INF-COEF          = 9.000000
    ///           porcentajeIncrementoU = 10.000000
    ///           NAME_CALENER      = ""
    ///           TransmisividadJulio = 1.000000
    ///           VIGENCIA = ( "A", "B", "C", "D", "E", "F")
    ///           IMAGE = ""
    ///           TRANSMITANCIA       =            5.7
    ///           SHADING-COEF        =           0.86
    ///           SHADE-COEF-SUMMER   =              1
    ///           SHADE-COEF-WINTER   =              1
    ///           MARKER-SUMMER       =              1
    ///           MARKER-WINTER       =              1
    ///           LIBRARY           =  NO
    ///           UTIL              =  YES
    ///           ISDOOR            = NO
    ///           DEFAULT           = NO
    ///          ..
    /// ```
    /// TODO: atributos no trasladados: ISDOOR, TRANSMITANCIA, SHADING-COEF
    /// TODO: SHADE-COEF-SUMMER, SHADE-COEF-WINTER, MARKER-SUMMER,  MARKER-WINTER,
    /// TODO: LIBRARY (proviene de la biblioteca?), UTIL (se utiliza?), DEFAULT
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        let group = attrs.remove_str("GROUP")?;
        let glass = attrs.remove_str("GLASS-TYPE")?;
        let glassgroup = attrs.remove_str("GROUP-GLASS")?;
        let frame = attrs.remove_str("NAME-FRAME")?;
        let framegroup = attrs.remove_str("GROUP-FRAME")?;
        let framepct = attrs.remove_f32("PORCENTAGE")?;
        let infcoeff = attrs.remove_f32("INF-COEF")?;
        let deltau = attrs
            .remove_f32("porcentajeIncrementoU")
            .unwrap_or_default();
        let gglshwi = attrs.remove_f32("TransmisividadJulio").ok();
        Ok(Self {
            name,
            group,
            glass,
            glassgroup,
            frame,
            framegroup,
            framepct,
            infcoeff,
            deltau,
            gglshwi,
        })
    }
}

/// Marco de hueco (NAME-FRAME)
#[derive(Debug, Clone, Default)]
pub struct Frame {
    /// Nombre
    pub name: String,
    /// Grupo al que pertenece (biblioteca)
    pub group: String,
    /// Transmitancia térmica, U (W/m2K)
    pub conductivity: f32,
    /// Absortividad del marco, alpha (-)
    pub absorptivity: f32,
    /// Ancho del marco (m)
    pub width: f32,
}

impl TryFrom<BdlBlock> for Frame {
    type Error = Error;

    /// Conversión de bloque BDL a marco de hueco (NAME-FRAME)
    ///
    /// Conductividad en FRAME-CONDUCT (W/m2K)
    /// Absortividad(alpha) en FRAME-ABS (-)
    ///
    /// Ejemplo:
    /// ```text
    ///      "Marco PVC_1" = NAME-FRAME
    ///      GROUP         = "Marcos HULC2020"
    ///      FRAME-WIDTH   =            0.1
    ///      FRAME-CONDUCT =            1.3
    ///      FRAME-ABS     =            0.7
    ///      NAME_CALENER  = ""
    ///      LIBRARY       = NO
    ///      UTIL          =  NO
    ///      ..
    /// ```
    /// TODO: Propiedades no trasladadas: NAME-CALENER, LIBRRARY, UTIL
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        let group = attrs.remove_str("GROUP")?;
        let conductivity = attrs.remove_f32("FRAME-CONDUCT")?;
        let absorptivity = attrs.remove_f32("FRAME-ABS")?;
        let width = attrs.remove_f32("FRAME-WIDTH")?;
        Ok(Self {
            name,
            group,
            conductivity,
            absorptivity,
            width,
        })
    }
}

/// Vidrio (GLASS-TYPE)
#[derive(Debug, Clone, Default)]
pub struct Glass {
    /// Nombre
    pub name: String,
    /// Grupo al que pertenece (biblioteca)
    pub group: String,
    /// Conductividad W/m2K (GLASS-CONDUCTANCE)
    pub conductivity: f32,
    /// Factor solar del vidrio a incidencia normal - (SHADING-COEF / 0.86)
    pub g_gln: f32,
}

impl TryFrom<BdlBlock> for Glass {
    type Error = Error;

    /// Conversión de bloque BDL a vidrio (GLASS-TYPE)
    ///
    /// Conductividad en GLASS-CONDUCTANCE (W/m2K)
    /// Factor solar (g) en SHADING-COEF * 0.85 (-)
    ///
    /// Ejemplo:
    /// ```text
    ///      "Vidrio Triple Bajo Emisivo" = GLASS-TYPE
    ///           GROUP             = "Vidrios HULC2020"
    ///           TYPE              = SHADING-COEF
    ///           SHADING-COEF      =      0.5882353
    ///           GLASS-CONDUCTANCE =           1.25
    ///           NAME_CALENER      = ""
    ///           LIBRARY       =  NO
    ///           UTIL          =  NO
    ///           ..
    ///      $ LIDER antiguo
    ///      "GT_referencia-3" = GLASS-TYPE
    ///           TYPE = SHADING-COEF
    ///           SHADING-COEF = 0
    ///           SHADING-COEF-SUMMER = 0
    ///           GLASS-CONDUCTANCE = 3.5
    ///          ..        
    /// ```
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        if attrs.remove_str("TYPE")? != "SHADING-COEF" {
            bail!(
                "Definición de vidrio por código no soportada en '{}'",
                &name
            );
        };
        // LIDER antiguo no guardaba el grupo
        let group = attrs.remove_str("GROUP").unwrap_or("Vidrios".to_string());
        let conductivity = attrs.remove_f32("GLASS-CONDUCTANCE")?;
        // El SHADING-COEF es SGHC/SGHC_ref donde:
        // - SGHC_ref = 0.86 (vidrio claro) (a veces se indica 0.87)
        // - SGHC es el factor solar del vidrio a incidencia normal
        // A nosotros nos interesa covertir este valor a g_gln,
        // y por tanto debemos multiplicar por 0.86
        let g_gln = attrs.remove_f32("SHADING-COEF")? * 0.86;
        Ok(Self {
            name,
            group,
            conductivity,
            g_gln,
        })
    }
}

/// Puente térmico (THERMAL-BRIDGE)
#[derive(Debug, Clone, Default)]
pub struct ThermalBridge {
    /// Nombre
    pub name: String,
    /// Longitud total (m)
    /// En LIDER antiguo no se guarda la medición en el objeto
    pub length: Option<f32>,
    /// Tipo de puente térmico:
    /// - PILLAR: pilar en fachada,
    /// - WINDOW-FRAME: borde de hueco,
    /// - SLAB: Forjado con cubierta o con suelo en contacto con el aire (anglemin, anglemax, partition)
    /// - MASONRY: Encuentros entre muros (anglemin, anglemax, partition)
    /// - UNDER-EXT: Solera con pared exterior (anglemin, anglemax, partition)
    pub tbtype: String,
    /// Transmitancia térmica W/mK
    pub psi: f32,
    /// Fractor de resistencia superficial frsi (condensaciones)
    pub frsi: f32,
    /// Propiedades geométricas de los encuentros (anglemin, anglemax, partition)
    pub geometry: Option<TBGeometry>,
    /// Datos para definición por catálogo (tipo 3)
    pub catalog: Option<TBByCatalog>,
}

/// Definición por usuario (definition 2)
#[derive(Debug, Clone, Default)]
pub struct TBGeometry {
    /// Tipo de encuentro entre elementos:
    /// - YES -> frente de forjado
    /// - BOTH -> encuentros entre dos particiones exteriores
    pub partition: String,
    /// Ángulo mínimo (grados sexagesimales)
    pub anglemin: f32,
    /// Ángulo máximo (grados sexagesimales)
    pub anglemax: f32,
}

/// Definición por catálogo (definition 3)
#[derive(Debug, Clone, Default)]
pub struct TBByCatalog {
    /// Lista de tipos
    pub classes: Vec<String>,
    /// Lista de porcentajes de la longitud total
    pub pcts: Vec<f32>,
    /// Lista de transmitancias del primer elemento del encuentro (muro) W/m2k
    pub firstelems: Vec<f32>,
    /// Lista de transmitancias del segundo elemento del encuentro (muro) W/m2k
    pub secondelems: Option<Vec<f32>>,
}

impl TryFrom<BdlBlock> for ThermalBridge {
    type Error = Error;

    /// Conversión de bloque BDL a puente térmico (THERMAL-BRIDGE)
    ///
    /// Se pueden de definir (DEFINICION) por defecto (1), por usuario (2) o por catálogo (3?)
    ///
    /// Ejemplo:
    /// ```text
    ///      "LONGITUDES_CALCULADAS" = THERMAL-BRIDGE
    ///            LONG-TOTAL = 0.000000
    ///            DEFINICION = 1
    ///          ..
    ///      "FRENTE_FORJADO" = THERMAL-BRIDGE
    ///            LONG-TOTAL = 171.629913
    ///            DEFINICION = 2
    ///            TTL    = 0.080000
    ///            FRSI        = 0.45
    ///            ANGLE-MIN   = 135
    ///            ANGLE-MAX   = 225
    ///            TYPE        = SLAB
    ///            PARTITION   = YES
    ///          ..
    ///     "UNION_CUBIERTA" = THERMAL-BRIDGE
    ///         LONG-TOTAL = 148.341034
    ///         DEFINICION = 3
    ///         TTL    = 0.226667
    ///         LISTA-N   = ( "Cubiertas planas - Forjado no interrumpe el aislamiento en fachada")
    ///         LISTA-L   = ( 100)
    ///         LISTA-MURO   = ( 0.230000)
    ///         LISTA-MARCO   = ( 0.200000)
    ///         FRSI        = 0.28
    ///         ANGLE-MIN   = 0
    ///         ANGLE-MAX   = 135
    ///         TYPE        = SLAB
    ///         PARTITION   = BOTH
    ///         ..
    /// ```
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        let length = attrs.remove_f32("LONG-TOTAL").ok();
        let (psi, frsi) = if name == "LONGITUDES_CALCULADAS" {
            (0.0, 0.0)
        } else {
            (attrs.remove_f32("TTL")?, attrs.remove_f32("FRSI")?)
        };
        let tbtype = attrs.remove_str("TYPE").ok().unwrap_or_default();
        let geometry = match tbtype.as_str() {
            "WINDOW-FRAME" | "PILLAR" | "" => None,
            _ => Some(TBGeometry {
                anglemin: attrs.remove_f32("ANGLE-MIN")?,
                anglemax: attrs.remove_f32("ANGLE-MAX")?,
                partition: attrs.remove_str("PARTITION")?,
            }),
        };

        let defn = attrs
            .remove_f32("DEFINICION")
            .and_then(|v| Ok(v as i32))
            .ok(); // El LIDER antiguo no usa la definición del tipo

        let catalog = match defn {
            // Definido con valor por defecto o por el usuario
            Some(1) | Some(2) | None => None,
            // Definido por catálogo de PTs
            Some(3) => Some(TBByCatalog {
                classes: extract_namesvec(attrs.remove_str("LISTA-N")?),
                pcts: extract_f32vec(attrs.remove_str("LISTA-L")?)?,
                firstelems: extract_f32vec(attrs.remove_str("LISTA-MURO")?)?,
                secondelems: if let Ok(list) = attrs.remove_str("LISTA-MARCO") {
                    Some(extract_f32vec(list)?)
                } else {
                    None
                },
            }),
            Some(v) => bail!("Puente térmico '{}' con tipo desconocido ({})", name, v),
        };
        Ok(Self {
            name,
            length,
            tbtype,
            psi,
            frsi,
            geometry,
            catalog,
        })
    }
}
