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
//! Composición constructiva de huecos (GAP)

use failure::Error;
use std::{collections::HashMap, convert::TryFrom};

use crate::bdl::{BdlBlock, Frame, Glass};

/// Definición de hueco o lucernario (GAP)
#[derive(Debug, Clone, Default)]
pub struct WindowCons {
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
    /// Fracción de hueco cubierto por el marco [0.0-1.0]
    pub framefrac: f32,
    /// Permeabilidad al aire (m3/hm2 a 100Pa)
    pub infcoeff: f32,
    /// Porcentaje de U debido a intercalarios y cajón de persiana (%)
    pub deltau: f32,
    /// Transmitancia total de energía del acristalameinto con los dispositivo de sombra móvil activados (g_gl;sh;wi) (-)
    pub gglshwi: Option<f32>,
}

// TODO: Llevar estas implementaciones a las WindowCons de types, que es donde deberían estar
// y dejar esto solo como contenedores de datos
impl WindowCons {
    /// Cálculo de U de la construcción del hueco
    /// Incluye las resistencias superficiales (que ya están consideradas en vidrio y marco, para sus posiciones)
    pub fn u(
        &self,
        framesdb: &HashMap<String, Frame>,
        glassesdb: &HashMap<String, Glass>,
    ) -> Result<f32, Error> {
        // Vidrio del hueco (Glass)
        let glass = glassesdb.get(&self.glass).ok_or_else(|| {
            format_err!(
                "Vidrio {} de la construcción {} no encontrado",
                self.glass,
                self.name
            )
        })?;
        // Marco del hueco (Frame)
        let frame = framesdb.get(&self.frame).ok_or_else(|| {
            format_err!(
                "Marco {} de la construcción {} no encontrado",
                self.frame,
                self.name
            )
        })?;
        let deltau = self.deltau; // deltau de persiana e intercalarios
        let frameu = frame.conductivity;
        let glassu = glass.conductivity;
        let u =
            (1.0 + deltau / 100.0) * (frameu * self.framefrac + glassu * (1.0 - self.framefrac));
        Ok(u)
    }
}

impl TryFrom<BdlBlock> for WindowCons {
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
        let framefrac = attrs.remove_f32("PORCENTAGE")? / 100.0;
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
            framefrac,
            infcoeff,
            deltau,
            gglshwi,
        })
    }
}