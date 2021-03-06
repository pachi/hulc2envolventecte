// Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>
// Distributed under the MIT License
// (See acoompanying LICENSE file or a copy at http://opensource.org/licenses/MIT)

//! Conversión desde CtehexmlData a cte::Model

use std::{collections::BTreeMap, convert::TryFrom, convert::TryInto};

use anyhow::{anyhow, format_err, Error};
use log::warn;

use crate::{
    bdl::{self, Data},
    parsers::ctehexml,
    utils::{fround2, fround3, orientation_bdl_to_52016, uuid_from_obj},
};

pub use super::{
    BoundaryType, Meta, Model, Orientation, Space, SpaceType, ThermalBridge, Tilt, Wall, WallCons,
    Window, WindowCons,
};

// Conversiones de BDL a tipos CTE -------------------

impl From<bdl::BoundaryType> for BoundaryType {
    fn from(boundary: bdl::BoundaryType) -> Self {
        match boundary {
            bdl::BoundaryType::EXTERIOR => Self::EXTERIOR,
            bdl::BoundaryType::INTERIOR => Self::INTERIOR,
            bdl::BoundaryType::GROUND => Self::GROUND,
            bdl::BoundaryType::ADIABATIC => Self::ADIABATIC,
        }
    }
}

impl TryFrom<&ctehexml::CtehexmlData> for Model {
    type Error = Error;
    fn try_from(d: &ctehexml::CtehexmlData) -> Result<Self, Self::Error> {
        let bdl = &d.bdldata;

        let mut walls = walls_from_bdl(&bdl)?;
        let mut windows = windows_from_bdl(&walls, &bdl);
        let thermal_bridges = thermal_bridges_from_bdl(&bdl);
        let wallcons = wallcons_from_bdl(&walls, &bdl)?;
        let wincons = windowcons_from_bdl(&bdl)?;
        let spaces = spaces_from_bdl(&bdl)?;

        // Cambia referencias a nombres por id's
        let spaceids = spaces
            .iter()
            .map(|s| (s.name.clone(), s.id.clone()))
            .collect::<BTreeMap<String, String>>();
        let wallids = walls
            .iter()
            .map(|s| (s.name.clone(), s.id.clone()))
            .collect::<BTreeMap<String, String>>();
        let wallconsids = wallcons
            .iter()
            .map(|s| (s.name.clone(), s.id.clone()))
            .collect::<BTreeMap<String, String>>();
        let winconsids = wincons
            .iter()
            .map(|s| (s.name.clone(), s.id.clone()))
            .collect::<BTreeMap<String, String>>();

        walls.iter_mut().for_each(|w| {
            w.cons = wallconsids.get(&w.cons).unwrap().to_owned();
            w.space = spaceids.get(&w.space).unwrap().to_owned();
            if let Some(ref nxt) = w.nextto {
                w.nextto = Some(spaceids.get(nxt).unwrap().to_owned())
            };
        });
        windows.iter_mut().for_each(|w| {
            w.cons = winconsids.get(&w.cons).unwrap().to_owned();
            w.wall = wallids.get(&w.wall).unwrap().to_owned();
        });

        // Completa metadatos desde ctehexml y el bdl
        // Desviación general respecto al Norte (criterio BDL)
        let buildparams = bdl.meta.get("BUILD-PARAMETERS").unwrap();
        let d_perim_insulation = buildparams
            .attrs
            .get_f32("D-AISLAMIENTO-PERIMETRAL")
            .unwrap_or(0.0);
        let rn_perim_insulation = buildparams
            .attrs
            .get_f32("RA-AISLAMIENTO-PERIMETRAL")
            .unwrap_or(0.0);

        let dg = &d.datos_generales;
        let is_dwelling =
            ["Unifamiliar", "Bloque", "UnaBloque"].contains(&dg.tipo_vivienda.as_str());

        let meta = Meta {
            name: dg.nombre_proyecto.clone(),
            is_new_building: dg.tipo_definicion.as_str() == "Nuevo",
            is_dwelling,
            num_dwellings: dg.num_viviendas_bloque,
            climate: dg
                .archivo_climatico
                .as_str()
                .try_into()
                .map_err(|e| anyhow!("ERROR: {}", e))?,
            global_ventilation_l_s: if is_dwelling {
                Some(dg.valor_impulsion_aire)
            } else {
                None
            },
            n50_test_ach: dg.valor_n50_medido,
            d_perim_insulation,
            rn_perim_insulation,
        };

        Ok(Model {
            meta,
            walls,
            windows,
            thermal_bridges,
            spaces,
            wincons,
            wallcons,
            extra: None,
        })
    }
}

/// Construye diccionario de espacios a partir de datos BDL (Data)
fn spaces_from_bdl(bdl: &Data) -> Result<Vec<Space>, Error> {
    bdl.spaces
        .iter()
        .map(|s| {
            let id = uuid_from_obj(&s);
            let area = fround2(s.area());
            let height = fround2(s.height);
            let exposed_perimeter = Some(fround2(s.exposed_perimeter(&bdl)));
            Ok(Space {
                id,
                name: s.name.clone(),
                area,
                z: s.z,
                exposed_perimeter,
                height,
                inside_tenv: s.insidete,
                multiplier: s.multiplier * s.floor_multiplier,
                space_type: match s.stype.as_ref() {
                    "CONDITIONED" => SpaceType::CONDITIONED,
                    "UNHABITED" => SpaceType::UNINHABITED,
                    _ => SpaceType::UNCONDITIONED,
                },
                n_v: s.airchanges_h,
            })
        })
        .collect::<Result<Vec<Space>, Error>>()
}

/// Construye muros de la envolvente a partir de datos BDL
fn walls_from_bdl(bdl: &Data) -> Result<Vec<Wall>, Error> {
    // Desviación general respecto al Norte (criterio BDL)
    let northangle = bdl
        .meta
        .get("BUILD-PARAMETERS")
        .unwrap()
        .attrs
        .get_f32("ANGLE")?;

    Ok(bdl
        .walls
        .iter()
        .map(|wall| -> Result<Wall, Error> {
            let id = uuid_from_obj(wall);
            let bounds = wall.bounds.into();
            let tilt = fround2(wall.tilt);
            Ok(Wall {
                id,
                name: wall.name.clone(),
                cons: wall.cons.to_string(),
                area: fround2(wall.net_area(bdl)?),
                space: wall.space.clone(),
                nextto: wall.nextto.clone(),
                bounds,
                azimuth: fround2(orientation_bdl_to_52016(wall.azimuth(northangle, &bdl)?)),
                tilt,
            })
        })
        .collect::<Result<Vec<Wall>, _>>()?)
}

/// Construye huecos de la envolvente a partir de datos BDL
fn windows_from_bdl(walls: &Vec<Wall>, bdl: &Data) -> Vec<Window> {
    bdl.windows
        .iter()
        .map(|win| {
            let id = uuid_from_obj(win);
            let wall = walls.iter().find(|w| w.name == win.wall).unwrap();
            let fshobst =
                fshobst_for_setback(wall.tilt, wall.azimuth, win.width, win.height, win.setback);
            Window {
                id,
                name: win.name.clone(),
                cons: win.cons.to_string(),
                wall: win.wall.clone(),
                area: fround2(win.width * win.height),
                fshobst: fround2(fshobst),
            }
        })
        .collect()
}

/// Construye puentes térmicos de la envolvente a partir de datos BDL
fn thermal_bridges_from_bdl(bdl: &Data) -> Vec<ThermalBridge> {
    // PTs
    bdl.tbridges
        .iter()
        .filter(|tb| tb.name != "LONGITUDES_CALCULADAS")
        .map(|tb| {
            let id = uuid_from_obj(tb);
            ThermalBridge {
                id,
                name: tb.name.clone(),
                l: fround2(tb.length.unwrap_or(0.0)),
                psi: tb.psi,
            }
        })
        .collect()
}

/// Construcciones de muros a partir de datos BDL
fn wallcons_from_bdl(walls: &Vec<Wall>, bdl: &Data) -> Result<Vec<WallCons>, Error> {
    let mut wcnames = walls
        .iter()
        .map(|w| w.cons.clone())
        .collect::<Vec<String>>();
    wcnames.sort();
    wcnames.dedup();

    wcnames
        .iter()
        .map(|wcons| {
            let wallcons = bdl
                .db
                .wallcons
                .get(wcons)
                .and_then(|cons|{
                    let id = uuid_from_obj(wcons);
                    match cons.r_intrinsic(&bdl.db.materials) {
                        Ok(r) => Some(WallCons {
                            id,
                            name: cons.name.clone(),
                            group: cons.group.clone(),
                            thickness: fround2(cons.total_thickness()),
                            r_intrinsic: fround3(r),
                            absorptance: cons.absorptance,
                        }),
                        _ => {
                            warn!(
                                "ERROR: No es posible calcular la R intrínseca de la construcción: {:?}\n",
                                cons,
                            );
                            None
                        }
                }})
                .ok_or_else(|| {
                    format_err!(
                        "Construcción de muro no encontrada o incorrecta: '{}'\n",
                        wcons,
                    )
                })?;
            Ok(wallcons)
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Construcciones de huecos a partir de datos BDL
fn windowcons_from_bdl(bdl: &Data) -> Result<Vec<WindowCons>, Error> {
    let mut wcnames: Vec<String> = bdl
        .windows
        .iter()
        .map(|w| w.cons.clone())
        .collect::<Vec<String>>();
    wcnames.sort();
    wcnames.dedup();

    wcnames
        .iter()
        .map(|wcons| {
            bdl.db
                .windowcons
                .get(wcons)
                .and_then(|cons| {
                    let id = uuid_from_obj(cons);
                    // Vidrio del hueco (Glass)
                    let glass = match bdl
                        .db
                        .glasses
                        .get(&cons.glass)
                        .ok_or_else(|| format_err!("Vidrio no encontrado: {}", cons.glass,))
                    {
                        Ok(glass) => glass,
                        _ => return None,
                    };
                    let ff = cons.framefrac;
                    let gglwi = fround2(glass.g_gln * 0.90);
                    let gglshwi = cons.gglshwi.unwrap_or(gglwi);
                    let infcoeff_100 = cons.infcoeff;
                    let u = fround2(cons.u(&bdl.db.frames, &bdl.db.glasses).unwrap_or_default());
                    Some(WindowCons {
                        id,
                        name: cons.name.clone(),
                        group: cons.group.clone(),
                        u,
                        ff,
                        gglwi,
                        gglshwi,
                        infcoeff_100,
                    })
                })
                .ok_or_else(|| {
                    format_err!(
                        "Construcción de hueco no encontrada o mal formada: {}",
                        &wcons,
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Factor de obstáculos remotos (Fshobst) en función del retranqueo, orientación y geometría del hueco
/// Se calcula, para huecos verticales, de acuerdo a la tabla 17 del DA DB-HE/1 (p. 19).
/// Es un cálculo best-effort. Podríamos mejorarlo implementando la 52016-1 pero lo puede personalizar el usuario luego
pub fn fshobst_for_setback(tilt: f32, azimuth: f32, width: f32, height: f32, setback: f32) -> f32 {
    use Orientation::*;
    use Tilt::*;

    // Calcular según orientación e inclinación
    let rh = setback / height;
    let rw = setback / width;
    match tilt.into() {
        // Elementos verticales - Tabla 17 del DA DB-HE/1 (p.19)
        SIDE => {
            let range_rh = if rh < 0.05 {
                0
            } else if rh <= 0.1 {
                1
            } else if rh <= 0.2 {
                2
            } else if rh <= 0.5 {
                3
            } else {
                4
            };
            let range_rw = if rw < 0.05 {
                0
            } else if rw <= 0.1 {
                1
            } else if rw <= 0.2 {
                2
            } else if rw <= 0.5 {
                3
            } else {
                4
            };
            match azimuth.into() {
                S => match (range_rh, range_rw) {
                    (1, 1) => 0.82,
                    (1, 2) => 0.74,
                    (1, 3) => 0.62,
                    (1, 4) => 0.39,
                    (2, 1) => 0.76,
                    (2, 2) => 0.67,
                    (2, 3) => 0.56,
                    (2, 4) => 0.35,
                    (3, 1) => 0.56,
                    (3, 2) => 0.51,
                    (3, 3) => 0.39,
                    (3, 4) => 0.27,
                    (4, 1) => 0.35,
                    (4, 2) => 0.32,
                    (4, 3) => 0.27,
                    (4, 4) => 0.17,
                    _ => 1.0,
                },
                SE | SW => match (range_rh, range_rw) {
                    (1, 1) => 0.86,
                    (1, 2) => 0.81,
                    (1, 3) => 0.72,
                    (1, 4) => 0.51,
                    (2, 1) => 0.79,
                    (2, 2) => 0.74,
                    (2, 3) => 0.66,
                    (2, 4) => 0.47,
                    (3, 1) => 0.59,
                    (3, 2) => 0.56,
                    (3, 3) => 0.47,
                    (3, 4) => 0.36,
                    (4, 1) => 0.38,
                    (4, 2) => 0.36,
                    (4, 3) => 0.32,
                    (4, 4) => 0.23,
                    _ => 1.0,
                },
                E | W => match (range_rh, range_rw) {
                    (1, 1) => 0.91,
                    (1, 2) => 0.87,
                    (1, 3) => 0.81,
                    (1, 4) => 0.65,
                    (2, 1) => 0.86,
                    (2, 2) => 0.82,
                    (2, 3) => 0.76,
                    (2, 4) => 0.61,
                    (3, 1) => 0.71,
                    (3, 2) => 0.68,
                    (3, 3) => 0.61,
                    (3, 4) => 0.51,
                    (4, 1) => 0.53,
                    (4, 2) => 0.51,
                    (4, 3) => 0.48,
                    (4, 4) => 0.39,
                    _ => 1.0,
                },
                _ => 1.0,
            }
        }
        TOP => {
            // Elementos horizontales: tabla 19 DA DB-HE/1 p.19
            let range_rh = if rh <= 0.1 {
                0
            } else if rh <= 0.5 {
                1
            } else if rh <= 1.0 {
                2
            } else if rh <= 2.0 {
                3
            } else if rh <= 5.0 {
                4
            } else {
                5
            };
            let range_rw = if rw <= 0.1 {
                0
            } else if rw <= 0.5 {
                1
            } else if rw <= 1.0 {
                2
            } else if rw <= 2.0 {
                3
            } else if rw <= 5.0 {
                4
            } else {
                5
            };
            let rmin = i32::min(range_rh, range_rw);
            let rmax = i32::max(range_rh, range_rw);
            match (rmax, rmin) {
                (0, 0) => 0.42,
                (1, 0) => 0.43,
                (1, 1) => 0.46,
                (2, 0) => 0.43,
                (2, 1) => 0.48,
                (2, 2) => 0.52,
                (3, 0) => 0.43,
                (3, 1) => 0.50,
                (3, 2) => 0.55,
                (3, 3) => 0.60,
                (4, 0) => 0.44,
                (4, 1) => 0.51,
                (4, 2) => 0.58,
                (4, 3) => 0.66,
                (4, 4) => 0.75,
                (5, 0) => 0.44,
                (5, 1) => 0.52,
                (5, 2) => 0.59,
                (5, 3) => 0.68,
                (5, 4) => 0.79,
                _ => 0.85,
            }
        }
        BOTTOM => 1.0,
    }
}
