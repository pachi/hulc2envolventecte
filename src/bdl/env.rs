//! Parser del Building Description Language (BDL) de DOE
//!
//! Elementos de la envolvente térmica:
//!
//! - Huecos (WINDOW)
//! - muros (EXTERIOR-WALL, INTERIOR-WALL, UNDERGROUND-WALL)
//! - cubiertas (ROOF)
//!
//! Todos menos el hueco tienen una construcción y pertenecen a un espacio (location)
//!
//! Otros elementos:
//! - Sombra (BUILDING-SHADE)

use failure::Error;
use std::convert::TryFrom;

use super::{geom::Vertex3D, BdlBlock, BdlData, window::Window};

/// Elementos de envolvente
#[derive(Debug)]
pub enum BdlEnvType {
    Window(Window),
    ExteriorWall(ExteriorWall),
    InteriorWall(InteriorWall),
    UndergroundWall(UndergroundWall),
    Roof(ExteriorWall),
}

// Muros (EXTERIOR-WALL, ROOF, INTERIOR-WALL, UNDERGROUND-WALL) ------------------

/// Definición geométrica de un muro (EXTERIOR-WALL, ROOF o INTERIOR-WALL)
/// Se usa cuando no se define respecto a un vértice del espacio padre sino por polígono
#[derive(Debug, Clone, Default)]
pub struct WallGeometry {
    /// Nombre del polígono que define la geometría
    pub polygon: String,
    /// Coordenada X de la esquina inferior izquierda
    pub x: f32,
    /// Coordenada Y de la esquina inferior izquierda
    pub y: f32,
    /// Coordenada Z de la esquina inferior izquierda
    pub z: f32,
    /// Acimut (grados sexagesimales)
    /// Ángulo entre el eje Y del espacio y la proyección horizontal de la normal exterior del muro
    pub azimuth: f32,
    /// Inclinación (grados sexagesimales)
    /// Ángulo entre el eje Z y la normal exterior del muro
    pub tilt: f32,
}

impl WallGeometry {
    pub fn parse_wallgeometry(
        mut attrs: super::AttrMap,
        btype: &str,
        location: &Option<String>,
    ) -> Result<Option<Self>, Error> {
        if let Ok(polygon) = attrs.remove_str("POLYGON") {
            let x = attrs.remove_f32("X")?;
            let y = attrs.remove_f32("Y")?;
            let z = attrs.remove_f32("Z")?;
            let azimuth = attrs.remove_f32("AZIMUTH")?;

            // Si la inclinación es None (se define location)
            // asignamos el valor por defecto, que es:
            // - Para btype = ROOF -> 0.0 (hacia arriba)
            // - Para el resto de btypes:
            //      - con location = TOP -> tilt = 0.0 (techo)
            //      - con location = BOTTOM -> tilt = 180.0 (suelo)
            //      - el resto -> tilt = 90.0 (defecto)
            let tilt = match attrs.remove_f32("TILT").ok() {
                Some(tilt) => tilt,
                _ => match (btype, location.as_deref()) {
                    ("ROOF", _) | (_, Some("TOP")) => 0.0,
                    (_, Some("BOTTOM")) => 180.0,
                    _ => 90.0,
                },
            };

            Ok(Some(WallGeometry {
                polygon,
                x,
                y,
                z,
                azimuth,
                tilt,
            }))
        } else {
            Ok(None)
        }
    }
}

// Cerramientos ------------------
// - muro (o cubierta o suelo) exterior (EXTERIOR-WALL) - alias para cubierta: ROOF
// - muro (o cubierta o suelo) interior (INTERIOR-WALL)
// - muro (o cubierta o suelo) enterrado (UNDERGROUND-WALL)

/// Trait con métodos compartidos por todos los cerramientos
pub trait WallExt {
    /// Geometría del cerramiento
    fn get_geometry(&self) -> Option<&WallGeometry>;
    /// Posición del cerramiento
    fn get_location(&self) -> Option<&str>;
    /// Espacio al que pertenece el cerramiento
    fn get_space(&self) -> &str;
    /// Nombre del cerramiento
    fn get_name(&self) -> &str;
    /// Tipo de cerramiento (ROOF, EXTERIOR-WALL, INTERIOR-WALL, UNDERGROUND-WALL)
    fn get_type(&self) -> &str;

    /// Localiza el vértice definido en location
    /// TODO: se podría amortizar este cálculo dejando el vértice ya al hacer el parsing
    fn get_location_vertex(&self) -> Option<String> {
        self.get_location().and_then(|l| {
            l.split('-')
                .collect::<Vec<_>>()
                .get(1)
                .map(|s| s.to_string())
        })
    }

    /// Superficie bruta (incluyendo huecos) del muro (m2)
    ///
    /// TODO: la búsqueda de polígonos y espacios no es óptima (se podría cachear)
    fn gross_area(&self, db: &BdlData) -> Result<f32, Error> {
        if let Some(geom) = &self.get_geometry() {
            // Superficie para muros definidos por polígono
            let geom_polygon = db.polygons.get(&geom.polygon).ok_or_else(|| {
                format_err!(
                    "Polígono del cerramiento {} no encontrado {}. No se puede calcular la superficie",
                    self.get_name(),
                    geom.polygon
                )
            })?;
            Ok(geom_polygon.area())
        } else if let Some(location) = &self.get_location() {
            // Superficie para muros definidos por posición, en un espacio
            let space = db.spaces.iter().find(|s| s.name == self.get_space()).ok_or_else(|| {
                format_err!(
                    "Espacio {} al que pertenece el cerramiento {} no encontrado. No se puede calcular la superficie",
                    self.get_space(),
                    self.get_name()
                )
            })?;
            // Elementos de suelo o techo
            if ["TOP", "BOTTOM"].contains(&location) {
                space.area(&db)
            // Elementos definidos por vértice
            } else {
                let vertex = self
                    .get_location_vertex()
                    .ok_or_else(|| {
                        format_err!(
                            "Vértice del cerramiento {} no encontrado en {}",
                            self.get_name(),
                            location
                        )
                    })?
                    .to_string();
                let poly = db.polygons.get(&space.polygon).ok_or_else(|| {
                    format_err!(
                        "Polígono {} del espacio {} al que pertenece el cerramiento {} no encontrado. No se puede calcular la superficie",
                        space.polygon,
                        self.get_space(),
                        self.get_name()
                    )
                })?;
                let height = space.height(&db)?;
                let length = poly.edge_length(&vertex);
                Ok(height * length)
            }
        } else {
            bail!("Formato de cerramiento incorrecto. No se define por polígono ni por vértice")
        }
    }

    /// Superficie neta (sin huecos) del cerramiento (m2)
    fn net_area(&self, db: &BdlData) -> Result<f32, Error> {
        let wall_gross_area = self.gross_area(db)?;
        let windows_area = db
            .env
            .iter()
            .filter(|e| {
                if let BdlEnvType::Window(win) = e {
                    win.wall == self.get_name()
                } else {
                    false
                }
            })
            .map(|w| match w {
                BdlEnvType::Window(win) => win.area(),
                _ => 0.0,
            })
            .sum::<f32>();
        Ok(wall_gross_area - windows_area)
    }

    /// Perímetro del cerramiento (m)
    fn perimeter(&self, db: &BdlData) -> Result<f32, Error> {
        unimplemented!()
    }

    /// Inclinación del cerramiento (grados)
    /// Ángulo de la normal del cerramiento con el eje Z
    fn tilt(&self) -> f32 {
        if let Some(geom) = self.get_geometry() {
            geom.tilt
        } else {
            match self.get_type() {
                "ROOF" => 0.0,
                _ => match self.get_location() {
                    Some("TOP") => 0.0,
                    Some("BOTTOM") => 180.0,
                    _ => 90.0,
                },
            }
        }
    }
}

// Muro exterior (EXTERIOR-WALL) o cubierta (ROOF) ------------------------------
// ROOF es igual pero con inclinación por defecto = 0 en vez de 90

/// Muro exterior (EXTERIOR-WALL) o cubierta (ROOF)
/// Puede definirse su configuración geométrica por polígono
/// o por localización respecto al espacio padre.
#[derive(Debug, Clone, Default)]
pub struct ExteriorWall {
    /// Nombre
    pub name: String,
    /// Espacio en al que pertenece el muro o cubierta
    pub space: String,
    /// Definición de la composición del cerramiento (Construction)
    pub construction: String,
    /// Posición respecto al espacio asociado (TOP, BOTTOM, nombreespacio)
    pub location: Option<String>,
    /// Posición definida por polígono
    pub geometry: Option<WallGeometry>,
    // --- Propiedades exclusivas de cerramientos exteriores ---
    /// Tipo (EXTERIOR-WALL o ROOF)
    pub wtype: String,
    /// Absortividad definida por usuario
    pub absorptance: f32,
}

impl WallExt for ExteriorWall {
    fn get_geometry(&self) -> Option<&WallGeometry> {
        self.geometry.as_ref()
    }
    fn get_location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    fn get_space(&self) -> &str {
        self.space.as_str()
    }
    fn get_name(&self) -> &str {
        self.name.as_str()
    }
    fn get_type(&self) -> &str {
        &self.wtype
    }
}

impl TryFrom<BdlBlock> for ExteriorWall {
    type Error = Error;

    /// Conversión de bloque BDL a elemento en contacto con el aire exterior
    /// (muro, cubierta o suelo)
    ///
    /// Ejemplos en BDL:
    /// ```text
    ///    "P01_E02_PE006" = EXTERIOR-WALL
    ///         ABSORPTANCE   =            0.6
    ///         COMPROBAR-REQUISITOS-MINIMOS = YES
    ///         TYPE_ABSORPTANCE    = 1
    ///         COLOR_ABSORPTANCE   = 0
    ///         DEGREE_ABSORPTANCE   = 2
    ///         CONSTRUCCION_MURO  = "muro_opaco"
    ///         CONSTRUCTION  = "muro_opaco0.60"
    ///         LOCATION      = SPACE-V11
    ///         ..
    ///     "P02_E01_FE001" = EXTERIOR-WALL
    ///         ABSORPTANCE   =           0.95
    ///         COMPROBAR-REQUISITOS-MINIMOS = YES
    ///         TYPE_ABSORPTANCE    = 0
    ///         COLOR_ABSORPTANCE   = 7
    ///         DEGREE_ABSORPTANCE   = 2
    ///         CONSTRUCCION_MURO  = "muro_opaco"  
    ///         CONSTRUCTION  = "muro_opaco0.95"  
    ///         X             =       -49.0098
    ///         Y             =              0
    ///         Z             =              0
    ///         AZIMUTH       =             90
    ///         TILT          =            180
    ///         POLYGON       = "P02_E01_FE001_Poligono3"
    ///         ..
    ///     "P03_E01_CUB001" = ROOF
    ///         ABSORPTANCE   =            0.6
    ///         COMPROBAR-REQUISITOS-MINIMOS = YES
    ///         TYPE_ABSORPTANCE    = 0
    ///         COLOR_ABSORPTANCE   = 0
    ///         DEGREE_ABSORPTANCE   = 2
    ///         CONSTRUCTION  = "cubierta"
    ///         LOCATION      = TOP
    ///         ..
    /// ```
    /// TODO: atributos no trasladados:
    /// TODO: propiedades para definir el estado de la interfaz para la selección de la absortividad:
    /// TODO: TYPE_ABSORPTANCE, COLOR_ABSORPTANCE, DEGREE_ABSORPTANCE,
    /// TODO: CONSTRUCCION_MURO, COMPROBAR-REQUISITOS-MINIMOS
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name,
            btype,
            parent,
            mut attrs,
            ..
        } = value;
        let space = parent.ok_or_else(|| {
            format_err!(
                "Elemento en contacto con el aire exterior sin espacio asociado '{}'",
                &name
            )
        })?;
        let construction = attrs.remove_str("CONSTRUCTION")?;
        let absorptance = attrs.remove_f32("ABSORPTANCE")?;
        let location = attrs.remove_str("LOCATION").ok();
        let geometry = WallGeometry::parse_wallgeometry(attrs, &btype, &location)?;
        Ok(Self {
            name,
            wtype: btype,
            space,
            construction,
            absorptance,
            location,
            geometry,
        })
    }
}

// Muro interior (INTERIOR-WALL) -------------------------------------

/// Muro interior
#[derive(Debug, Clone, Default)]
pub struct InteriorWall {
    /// Nombre
    pub name: String,
    /// Espacio en al que pertenece el muro
    pub space: String,
    /// Definición de la composición del cerramiento (Construction)
    pub construction: String,
    /// Posición respecto al espacio asociado (TOP, BOTTOM, nombreespacio)
    pub location: Option<String>,
    /// Posición definida por polígono
    pub geometry: Option<WallGeometry>,
    // --- Propiedades exclusivas de cerramientos interiores ---
    /// Tipo de muro interior
    /// - STANDARD: muro entre dos espacios
    /// - ADIABATIC: muro que no conduce calor (a otro espacio) pero lo almacena
    /// - INTERNAL: muro interior a un espacio (no comunica espacios)
    /// - AIR: superficie interior sin masa pero que admite convección
    pub wtype: String,
    /// Espacio adyacente que conecta con el espacio padre (salvo que sea adiabático o interior)
    pub nextto: Option<String>,
}

impl WallExt for InteriorWall {
    fn get_geometry(&self) -> Option<&WallGeometry> {
        self.geometry.as_ref()
    }
    fn get_location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    fn get_space(&self) -> &str {
        self.space.as_str()
    }
    fn get_name(&self) -> &str {
        self.name.as_str()
    }
    fn get_type(&self) -> &str {
        &self.wtype
    }
}

impl TryFrom<BdlBlock> for InteriorWall {
    type Error = Error;

    /// Conversión de bloque BDL a muro exterior (o cubierta)
    ///
    /// Ejemplos en BDL:
    /// ```text
    ///    "P01_E02_Med001" = INTERIOR-WALL
    ///         INT-WALL-TYPE = STANDARD
    ///         NEXT-TO       = "P01_E07"
    ///         COMPROBAR-REQUISITOS-MINIMOS = NO
    ///         CONSTRUCTION  = "tabique"
    ///         LOCATION      = SPACE-V1
    ///         ..
    ///     "P02_E01_FI002" = INTERIOR-WALL
    ///         INT-WALL-TYPE = STANDARD  
    ///         NEXT-TO       = "P01_E04"  
    ///         COMPROBAR-REQUISITOS-MINIMOS = NO
    ///         CONSTRUCTION  = "forjado_interior"                 
    ///         X             =         -38.33
    ///         Y             =           3.63
    ///         Z             =              0
    ///         AZIMUTH       =             90
    ///         TILT          =            180
    ///         POLYGON       = "P02_E01_FI002_Poligono2"
    ///         ..
    /// ```
    /// TODO: atributos no trasladados:
    /// TODO: COMPROBAR-REQUISITOS-MINIMOS
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name,
            btype,
            parent,
            mut attrs,
            ..
        } = value;
        let space =
            parent.ok_or_else(|| format_err!("Muro interior sin espacio asociado '{}'", &name))?;
        let wtype = attrs.remove_str("INT-WALL-TYPE")?;
        let nextto = attrs.remove_str("NEXT-TO").ok();
        let construction = attrs.remove_str("CONSTRUCTION")?;
        let location = attrs.remove_str("LOCATION").ok();
        let geometry = WallGeometry::parse_wallgeometry(attrs, &btype, &location)?;
        Ok(Self {
            name,
            wtype,
            nextto,
            space,
            construction,
            location,
            geometry,
        })
    }
}

// Muro o soleras en contacto con el terreno (UNDERGROUND-WALL) --------
//
// Ejemplo en BDL:
// ```text
//    "P01_E01_FTER001" = UNDERGROUND-WALL
//     Z-GROUND      =              0
//     COMPROBAR-REQUISITOS-MINIMOS = YES
//                    CONSTRUCTION  = "solera tipo"
//                    LOCATION      = BOTTOM
//                     AREA          =        418.4805
//                     PERIMETRO     =        65.25978
//                          ..
//                    "solera tipo" =  CONSTRUCTION
//                          TYPE   = LAYERS
//                          LAYERS = "solera tipo"
//                          ..
// ```

/// Muro (UNDERGROUND-WALL) o suelo (UNDEGROUND-FLOOR) en contacto con el terreno
#[derive(Debug, Clone, Default)]
pub struct UndergroundWall {
    /// Nombre
    pub name: String,
    /// Espacio en al que pertenece el muro o suelo
    pub space: String,
    /// Definición de la composición del cerramiento (Construction)
    pub construction: String,
    /// Posición respecto al espacio asociado (TOP, BOTTOM, nombreespacio)
    pub location: Option<String>,
    /// Posición definida por polígono
    pub geometry: Option<WallGeometry>,
    // --- Propiedades exclusivas de cerramientos enterrados ---
    /// Profundidad del elemento (m)
    pub zground: f32,
    // XXX: esto parece algo que guarda HULC pero se puede calcular
    /// Superficie (m2)
    pub area: Option<f32>,
    // XXX: esto parece algo que guarda HULC pero se puede calcular
    /// Perímetro (m)
    pub perimeter: Option<f32>,
}

impl WallExt for UndergroundWall {
    fn get_geometry(&self) -> Option<&WallGeometry> {
        self.geometry.as_ref()
    }
    fn get_location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    fn get_space(&self) -> &str {
        self.space.as_str()
    }
    fn get_name(&self) -> &str {
        self.name.as_str()
    }
    fn get_type(&self) -> &str {
        "UNDERGROUND-WALL"
    }
}

impl TryFrom<BdlBlock> for UndergroundWall {
    type Error = Error;

    /// Conversión de bloque BDL a muro exterior, suelo o cubierta enterrado
    ///
    /// Ejemplo en BDL:
    /// ```text
    ///    "P01_E01_FTER001" = UNDERGROUND-WALL
    ///         Z-GROUND      =              0
    ///         COMPROBAR-REQUISITOS-MINIMOS = YES
    ///         CONSTRUCTION  = "solera tipo"
    ///         LOCATION      = BOTTOM
    ///         AREA          =        418.4805
    ///         PERIMETRO     =        65.25978
    ///         ..
    ///    "P01_E01_TER002" = UNDERGROUND-WALL
    ///         Z-GROUND      =              0
    ///         COMPROBAR-REQUISITOS-MINIMOS = YES
    ///         CONSTRUCTION  = "Solera"  
    ///         LOCATION      = SPACE-V2  
    ///         ..
    /// ```
    /// TODO: atributos no trasladados:
    /// TODO: COMPROBAR-REQUISITOS-MINIMOS
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name,
            btype,
            parent,
            mut attrs,
            ..
        } = value;
        let zground = attrs.remove_f32("Z-GROUND")?;
        let area = attrs.remove_f32("AREA").ok();
        let perimeter = attrs.remove_f32("PERIMETRO").ok();
        let space =
            parent.ok_or_else(|| format_err!("Muro interior sin espacio asociado '{}'", &name))?;
        let construction = attrs.remove_str("CONSTRUCTION")?;
        let location = attrs.remove_str("LOCATION").ok();
        let geometry = WallGeometry::parse_wallgeometry(attrs, &btype, &location)?;
        Ok(Self {
            name,
            zground,
            area,
            perimeter,
            space,
            construction,
            location,
            geometry,
        })
    }
}

// Sombras ---------------------

/// Defininición de gometría de sombra como rectángulo
#[derive(Debug, Clone, Default)]
pub struct ShadeGeometry {
    /// Coordenada X de la esquina inferior izquierda
    pub x: f32,
    /// Coordenada Y de la esquina inferior izquierda
    pub y: f32,
    /// Coordenada Z de la esquina inferior izquierda
    pub z: f32,
    /// Alto, en eje Y local de la superficie (m)
    pub height: f32,
    /// Ancho, en eje X local de la superficie (m)
    pub width: f32,
    /// Acimut (grados sexagesimales)
    /// Ángulo entre el eje Y del espacio y la proyección horizontal de la normal exterior del plano
    pub azimuth: f32,
    /// Inclinación (grados sexagesimales)
    /// Ángulo entre el eje Z del edificio y la proyección de la normal exterior del plano
    pub tilt: f32,
}

/// Sombra (BUILDING-SHADE)
#[derive(Debug, Clone, Default)]
pub struct Shade {
    /// Nombre
    pub name: String,
    /// Transmisividad de la radiación solar de la superficie (-)
    pub tran: f32,
    /// Reflectividad visible de la superficie (-)
    pub refl: f32,
    /// Geometría por rectángulos
    pub geometry: Option<ShadeGeometry>,
    /// Geometría por vértices
    pub vertices: Option<Vec<Vertex3D>>,
}

impl TryFrom<BdlBlock> for Shade {
    type Error = Error;

    /// Conversión de bloque BDL a sombra
    ///
    /// Ejemplo en BDL:
    /// ```text
    ///     "patio1_lateral2" = BUILDING-SHADE
    ///         BULB-TRA = "Default.bulb"
    ///         BULB-REF = "Default.bulb"
    ///         TRAN     =              0
    ///         REFL     =            0.7
    ///         X        = 18.200001
    ///         Y        = 9.030000
    ///         Z        = 0.000000
    ///         HEIGHT   = 12.500000
    ///         WIDTH    = 3.500000
    ///         TILT     = 90.000000
    ///         AZIMUTH  = 180.000000
    ///         ..
    ///     "Sombra016" = BUILDING-SHADE
    ///         BULB-TRA = "Default.bulb"
    ///         BULB-REF = "Default.bulb"
    ///         TRAN     =              0
    ///         REFL     =            0.7
    ///         V1       =( 9.11, 25.7901, 12.5 )
    ///         V2       =( 9.11, 27.04, 12.5 )
    ///         V3       =( 6, 27.04, 12.5 )
    ///         V4       =( 6, 25.7901, 12.5 )
    ///         ..
    /// ```
    /// TODO: atributos no trasladados:
    /// TODO: BULB-TRA, BULB-REF
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name, mut attrs, ..
        } = value;
        let tran = attrs.remove_f32("TRAN")?;
        let refl = attrs.remove_f32("REFL")?;
        let (geometry, vertices) = if attrs.get_f32("X").is_ok() {
            // Definición por rectángulo
            (
                Some(ShadeGeometry {
                    x: attrs.remove_f32("X")?,
                    y: attrs.remove_f32("Y")?,
                    z: attrs.remove_f32("Z")?,
                    height: attrs.remove_f32("HEIGHT")?,
                    width: attrs.remove_f32("WIDTH")?,
                    azimuth: attrs.remove_f32("AZIMUTH")?,
                    tilt: attrs.remove_f32("TILT")?,
                }),
                None,
            )
        } else {
            // Definición por vértices
            let mut verts = Vec::new();
            for i in 1.. {
                let name = format!("V{}", i);
                if let Ok(vdata) = attrs.remove_str(&name) {
                    verts.push(Vertex3D {
                        name,
                        vector: vdata.parse()?,
                    });
                } else {
                    break;
                }
            }
            (None, Some(verts))
        };

        Ok(Self {
            name,
            tran,
            refl,
            geometry,
            vertices,
        })
    }
}
