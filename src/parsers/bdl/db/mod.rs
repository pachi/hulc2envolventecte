// Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>
// Distributed under the MIT License
// (See acoompanying LICENSE file or a copy at http://opensource.org/licenses/MIT)

//! Parser del Building Description Language (BDL) de DOE
//!
//! ## Base de datos de materiales y composiciones de elementos de la envolvente
//!
//! Los materiales se organizan por familias, dentro de ellas, por grupos:
//!
//! - Opacos
//!      - Materiales y productos (MATERIAL (tipo PROPERTIES o RESISTANCE))
//!      - Cerramientos y particiones (LAYERS)
//! - Semitransparentes
//!      - Vidrios (GLASS-TYPE)
//!      - Marcos (NAME-FRAME)
//!      - Composición de huecos y lucernarios (GAP)
//! - Puentes térmicos (THERMAL-BRIDGE)?

use std::collections::HashMap;

mod construction;
mod frame;
mod glass;
mod material;
mod wallcons;
mod windowcons;

pub use construction::Construction;
pub use frame::Frame;
pub use glass::Glass;
pub use material::Material;
pub use material::MaterialProperties;
pub use wallcons::WallCons;
pub use windowcons::WindowCons;

/// Elementos constructivos y de materiales pertenecientes a la base de datos
/// Se organizan por nombre y grupo (tipo)
#[derive(Debug, Clone, Default)]
pub struct DB {
    /// Material o producto
    pub materials: HashMap<String, Material>,
    /// Composición por capas (opacos)
    pub wallcons: HashMap<String, WallCons>,
    /// Composición por capas (huecos)
    pub windowcons: HashMap<String, WindowCons>,
    /// Vidrio
    pub glasses: HashMap<String, Glass>,
    /// Marco
    pub frames: HashMap<String, Frame>,
}
