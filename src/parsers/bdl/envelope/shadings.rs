/* -*- coding: utf-8 -*-

Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

//! Parser del Building Description Language (BDL) de DOE
//!
//! Elementos de sombra (BUILDING-SHADE)

use failure::Error;
use std::convert::TryFrom;

use crate::bdl::{envelope::Vertex3D, BdlBlock};

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