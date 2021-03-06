// Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>
// Distributed under the MIT License
// (See acoompanying LICENSE file or a copy at http://opensource.org/licenses/MIT)

//! Parser del Building Description Language (BDL) de DOE
//!
//! Elementos WINDOW de la envolvente térmica

use std::convert::TryFrom;

use anyhow::{bail, format_err, Error};

use crate::bdl::{extract_f32vec, BdlBlock, Data};

// Hueco (WINDOW) -------------------------------------------------

/// Hueco o lucernario (WINDOW)
#[derive(Debug, Clone, Default)]
pub struct Window {
    /// Nombre
    pub name: String,
    /// Muro, cubierta o suelo en el que se sitúa
    pub wall: String,
    /// Definición de la composición del hueco (WindowCons::name)
    pub cons: String,
    /// Distancia (m) del borde izquierdo del hueco al borde izquierdo del cerramiento que lo contiene (mirando desde fuera)
    pub x: f32,
    /// Distancia (m) del borde inferior del hueco al borde inferior del cerramiento que lo contiene (mirando desde fuera)
    pub y: f32,
    /// Altura del hueco (m)
    pub height: f32,
    /// Anchura del hueco (m)
    pub width: f32,
    /// Retranqueo del hueco (m)
    pub setback: f32,
    /// Coeficientes de corrección por dispositivo de sombra estacional
    /// - 0: Corrección de factor solar fuera de la temporada veraniega (-)
    /// - 1: Corrección de factor solar dentro de la temporada veraniega (-)
    /// - 2: Corrección de transmitancia térmica fuera de la temporada veraniega (-)
    /// - 3: Corrección de transmitancia térmica dentro de la temporada veraniega (-)
    pub coefs: Option<Vec<f32>>,
}

// TODO: Muchas de estas cosas seguramente tendrían que ir a types y quedar Window como datos simplemente
impl Window {
    /// Superficie de la ventana [m2]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Inclinación de la ventana (grados)
    /// Es el ángulo respecto al eje Z de la normal a la superficie en la que está la ventana
    pub fn tilt(&self, db: &Data) -> Result<f32, Error> {
        let wall = db.get_wall(&self.wall).ok_or_else(|| {
            format_err!(
                "Muro {} al que pertenece la ventana {} no encontrado. No se puede calcular la inclinación",
                self.wall,
                self.name
            )
        })?;
        Ok(wall.tilt)
    }

    /// Azimut de la ventana (grados)
    /// Es el ángulo respecto al eje Z de la normal a la superficie en la que está la ventana
    pub fn azimuth(&self, northangle: f32, db: &Data) -> Result<f32, Error> {
        let wall = db.get_wall(&self.wall).ok_or_else(|| {
            format_err!(
                "Muro {} al que pertenece la ventana {} no encontrado. No se puede calcular el azimut",
                self.wall,
                self.name
            )
        })?;
        wall.azimuth(northangle, db)
    }

    /// Perímetro del hueco [m]
    pub fn perimeter(&self) -> f32 {
        2.0 * (self.width + self.height)
    }
}

impl TryFrom<BdlBlock> for Window {
    type Error = Error;

    /// Conversión de bloque BDL a hueco o lucernario (WINDOW)
    ///
    /// ¿Puede definirse con GLASS-TYPE, WINDOW-LAYER o GAP?
    /// y puede pertenecer a un INTERIOR-WALL o EXTERIOR-WALL
    /// (trasnmisividadJulio)
    /// XXX:
    /// COEFF son los factores (f1, f2, f3, f4), donde f1 y f2 son los correctores del
    /// factor solar (fuera de la temporada de activación de las sombras estacionales y dentro de esa temporada)
    /// y f3 y f4 los correctores de la transmitancia térmica del hueco en las mismas temporadas
    /// (desactivado y con la sombra estacional activada)
    /// XXX: las propiedades del marco y vidrio se consultan a través del GAP
    ///
    /// Ejemplo en BDL:
    /// ```text
    ///     "P01_E02_PE005_V" = WINDOW
    ///     X              =            0.2
    ///     Y              =            0.1
    ///     SETBACK        =              0
    ///     HEIGHT         =            2.6
    ///     WIDTH          =              5
    ///     GAP            = "muro_cortina_controlsolar"
    ///     COEFF = ( 1.000000, 1.000000, 1.000000, 1.000000)
    ///     transmisividadJulio        = 0.220000
    ///     GLASS-TYPE     = "Doble baja emisividad argon"
    ///     FRAME-WIDTH   =      0.1329403
    ///     FRAME-CONDUCT =       5.299999
    ///     FRAME-ABS     =            0.7
    ///     INF-COEF       =              9
    ///     OVERHANG-A     =              0
    ///     OVERHANG-B     =              0
    ///     OVERHANG-W     =              0
    ///     OVERHANG-D     =              0
    ///     OVERHANG-ANGLE =              0
    ///     LEFT-FIN-A     =              0
    ///     LEFT-FIN-B     =              0
    ///     LEFT-FIN-H     =              0
    ///     LEFT-FIN-D     =              0
    ///     RIGHT-FIN-A    =              0
    ///     RIGHT-FIN-B    =              0
    ///     RIGHT-FIN-H    =              0
    ///     RIGHT-FIN-D    =              0
    ///     ..
    /// ```
    /// TODO: atributos no trasladados:
    /// TODO:  GLASS-TYPE, FRAME-WIDTH, FRAME-CONDUCT, FRAME-ABS, INF-COEF,
    /// TODO: propiedades para definir salientes y voladizos o para lamas:
    /// TODO: OVERHANG-A, OVERHANG-B, OVERHANG-W, OVERHANG-D, OVERHANG-ANGLE,
    /// TODO: LEFT-FIN-A, LEFT-FIN-B, LEFT-FIN-H, LEFT-FIN-D, RIGHT-FIN-A, RIGHT-FIN-B, RIGHT-FIN-H, RIGHT-FIN-D
    /// TODO: propiedades para definición de lamas
    fn try_from(value: BdlBlock) -> Result<Self, Self::Error> {
        let BdlBlock {
            name,
            parent,
            mut attrs,
            ..
        } = value;
        let wall = parent.ok_or_else(|| format_err!("Hueco sin muro asociado '{}'", &name))?;
        let cons = attrs.remove_str("GAP")?;
        let x = attrs.remove_f32("X")?;
        let y = attrs.remove_f32("Y")?;
        let height = attrs.remove_f32("HEIGHT")?;
        let width = attrs.remove_f32("WIDTH")?;
        let setback = attrs.remove_f32("SETBACK")?;
        let coefs = match attrs.remove_str("COEFF").ok() {
            None => None, // LIDER antiguo no define estos parámetros
            Some(vals) => match extract_f32vec(vals) {
                Ok(vec) if vec.len() == 4 => Some(vec),
                _ => bail!(
                    "Definición incorrecta de coeficientes de corrección en el hueco '{}'",
                    name
                ),
            },
        };

        Ok(Self {
            name,
            wall,
            cons,
            x,
            y,
            height,
            width,
            setback,
            coefs,
        })
    }
}
