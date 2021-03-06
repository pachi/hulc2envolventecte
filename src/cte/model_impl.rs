// Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>
// Distributed under the MIT License
// (See acoompanying LICENSE file or a copy at http://opensource.org/licenses/MIT)

//! Implementación del cálculo de la U de una composión constructiva de opaco, según su posición
//! - UNE-EN ISO 13789:2010 transmisión general
//! - UNE-EN ISO 6946:2012 para elementos opacos
//! - UNE-EN ISO 13770:2017 para elementos en contacto con el terremo
#![allow(non_snake_case)]

use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
};

use log::{debug, info, warn};

use super::{
    BoundaryType, KDetail, Model, N50HEDetail, Orientation, Space, SpaceType, Tilt, Wall, WallCons,
    Warning, WarningLevel, Window, WindowCons,
};
use crate::utils::fround2;

// Resistencias superficiales UNE-EN ISO 6946 [m2·K/W]
const RSI_ASCENDENTE: f32 = 0.10;
const RSI_HORIZONTAL: f32 = 0.13;
const RSI_DESCENDENTE: f32 = 0.17;
const RSE: f32 = 0.04;
// conductividad del terreno no helado, en [W/(m·K)]
const LAMBDA_GND: f32 = 2.0;
const LAMBDA_INS: f32 = 0.035;

impl Model {
    /// Localiza espacio
    pub fn get_space<'a>(&'a self, spaceid: &'a str) -> Option<&'a Space> {
        self.spaces.iter().find(|s| s.id == spaceid)
    }

    /// Localiza espacio por nombre
    pub fn get_space_by_name<'a>(&'a self, spacename: &'a str) -> Option<&'a Space> {
        self.spaces.iter().find(|s| s.name == spacename)
    }

    /// Localiza opaco
    pub fn get_wall<'a>(&'a self, wallid: &'a str) -> Option<&'a Wall> {
        self.walls.iter().find(|w| w.id == wallid)
    }

    /// Localiza espacio de opaco
    pub fn get_wallspace<'a>(&'a self, wall: &'a Wall) -> Option<&'a Space> {
        let maybespace = self.get_space(&wall.space);
        if maybespace.is_none() {
            warn!(
                "Muro {} ({}) con definición de espacio incorrecta {}",
                wall.id, wall.name, wall.space
            );
        }
        maybespace
    }

    /// Localiza opaco por nombre
    pub fn get_wall_by_name<'a>(&'a self, wallname: &'a str) -> Option<&'a Wall> {
        self.walls.iter().find(|w| w.name == wallname)
    }

    /// Localiza construcción de opaco
    pub fn get_wallcons<'a>(&'a self, wall: &'a Wall) -> Option<&'a WallCons> {
        let maybecons = self.wallcons.iter().find(|wc| wc.id == wall.cons);
        if maybecons.is_none() {
            warn!(
                "Muro {} ({}) con definición de construcción incorrecta {}",
                wall.id, wall.name, wall.cons
            );
        };
        maybecons
    }

    /// Localiza muro de hueco
    pub fn get_windowwall<'a>(&'a self, window: &'a Window) -> Option<&'a Wall> {
        let maybewall = self.get_wall(&window.wall);
        if maybewall.is_none() {
            warn!(
                "Hueco {} ({}) con definición de muro incorrecta {}",
                window.id, window.name, window.wall
            );
        }
        maybewall
    }

    /// Localiza construcción de hueco
    pub fn get_wincons<'a>(&'a self, win: &'a Window) -> Option<&'a WindowCons> {
        let maybecons = self.wincons.iter().find(|wc| wc.id == win.cons);
        if maybecons.is_none() {
            warn!(
                "Hueco {}({}) con definición de construcción incorrecta {}",
                win.id, win.name, win.cons
            );
        }
        maybecons
    }

    /// Iterador de los huecos pertenecientes a un muro
    pub fn windows_of_wall<'a>(&'a self, wallid: &'a str) -> impl Iterator<Item = &'a Window> {
        self.windows.iter().filter(move |w| w.wall == wallid)
    }

    /// Iterador de los cerramientos (incluyendo muros, suelos y techos) que delimitan un espacio
    pub fn walls_of_space<'a>(&'a self, spaceid: &'a str) -> impl Iterator<Item = &'a Wall> {
        self.walls.iter().filter(move |w| {
            w.space == spaceid
                || (if let Some(ref spc) = w.nextto {
                    spc == spaceid
                } else {
                    false
                })
        })
    }

    /// Iterador de los cerramientos de la envolvente térmica en contacto con el aire o el terreno
    /// Se excluyen los opacos sin espacio definido
    pub fn walls_of_envelope(&self) -> impl Iterator<Item = &Wall> {
        self.walls
            .iter()
            .filter(|w| [BoundaryType::EXTERIOR, BoundaryType::GROUND].contains(&w.bounds))
            .filter(move |w| {
                // Si el espacio no está definido se considera que no pertenece a la envolvente
                self.get_space(&w.space)
                    .map(|s| s.inside_tenv)
                    .unwrap_or(false)
            })
    }

    /// Iterador de los huecos de la envolvente térmica en contacto con el aire exterior
    /// Se excluyen los huecos sin espacio definido
    pub fn windows_of_envelope(&self) -> impl Iterator<Item = &Window> {
        self.walls
            .iter()
            .filter(|w| w.bounds == BoundaryType::EXTERIOR)
            .filter(move |w| {
                self.get_space(&w.space)
                    .map(|s| s.inside_tenv)
                    .unwrap_or(false)
            })
            .flat_map(move |wall| self.windows.iter().filter(move |w| w.wall == wall.id))
    }

    /// Calcula la superficie útil de los espacios habitables de la envolvente térmica [m²]
    pub fn a_ref(&self) -> f32 {
        let a_util: f32 = self
            .spaces
            .iter()
            .filter_map(|s| {
                if s.inside_tenv && s.space_type != SpaceType::UNINHABITED {
                    Some(s.area * s.multiplier)
                } else {
                    None
                }
            })
            .sum();
        fround2(a_util)
    }

    /// Calcula el volumen bruto de los espacios de la envolvente [m³]
    /// Computa el volumen de todos los espacios (habitables o no) de la envolvente
    pub fn vol_env_gross(&self) -> f32 {
        let v_env: f32 = self
            .spaces
            .iter()
            .filter_map(|s| {
                if s.inside_tenv {
                    Some(s.area * s.height * s.multiplier)
                } else {
                    None
                }
            })
            .sum();
        fround2(v_env)
    }
    /// Calcula el volumen neto de los espacios de la envolvente [m³]
    /// Computa el volumen de todos los espacios (habitables o no) de la envolvente y
    /// descuenta los volúmenes de forjados y cubiertas
    pub fn vol_env_net(&self) -> f32 {
        let v_env: f32 = self
            .spaces
            .iter()
            .filter_map(|s| {
                if s.inside_tenv {
                    Some(s.area * (s.height - self.top_wall_thickness(&s.id)) * s.multiplier)
                } else {
                    None
                }
            })
            .sum();
        fround2(v_env)
    }
    /// Calcula el volumen neto de los espacios habitables de la envolvente [m³]
    /// Computa el volumen de todos los espacios (solo habitables) de la envolvente y
    /// descuenta los volúmenes de forjados y cubiertas
    pub fn vol_env_inh_net(&self) -> f32 {
        let v_env: f32 = self
            .spaces
            .iter()
            .filter_map(|s| {
                if s.inside_tenv && s.space_type != SpaceType::UNINHABITED {
                    Some(s.area * (s.height - self.top_wall_thickness(&s.id)) * s.multiplier)
                } else {
                    None
                }
            })
            .sum();
        fround2(v_env)
    }

    /// Calcula la compacidad de la envolvente térmica del edificio V/A (m³/m²)
    /// De acuerdo con la definición del DB-HE comprende el volumen interior de la envolvente térmica (V)
    /// y la superficie de muros y huecos con intercambio térmico con el aire exterior o el terreno (A)
    /// Esta superficie tiene en cuenta los multiplicadores de espacios
    /// Se excluyen los huecos sin muro definido y los muros sin espacio definido
    /// Para area expuesta => compacidad = 0.0
    pub fn compacity(&self) -> f32 {
        let vol: f32 = self.vol_env_gross();
        let area: f32 = self
            .walls_of_envelope()
            .map(|w| {
                let multiplier = self.get_wallspace(&w).map(|s| s.multiplier).unwrap_or(1.0);
                let win_area: f32 = self.windows_of_wall(&w.id).map(|win| win.area).sum();
                (w.area + win_area) * multiplier
            })
            .sum();
        let compac = if area == 0.0 { 0.0 } else { vol / area };
        info!("V/A={:.2} m³/m², V={:.2} m³, A={:.2} m²", compac, vol, area);
        compac
    }

    /// Permeabilidad de opacos calculada según criterio de edad por defecto DB-HE2019 (1/h)
    /// TODO: usamos is_new_building pero igual merecería la pena una variable para permeabilidad mejorada
    pub fn C_o_he2019(&self) -> f32 {
        if self.meta.is_new_building {
            16.0
        } else {
            29.0
        }
    }

    /// Permeabilidad de opacos por defecto o, si hay ensayo de permeabilidad, el resultante del ensayo
    pub fn C_o(&self) -> f32 {
        if let Some(n50test) = self.meta.n50_test_ach {
            self.wall_inf_100_from_n50(n50test)
        } else {
            self.C_o_he2019()
        }
    }

    /// Devuelve valor de la relación de cambio de aire por defecto o, en su caso, de ensayo
    pub fn n50(&self) -> f32 {
        if let Some(n50test) = self.meta.n50_test_ach {
            n50test
        } else {
            self.n50_he2019().n50
        }
    }

    /// Calcula la tasa teórica de intercambio de aire a 50Pa según DB-HE2019 (1/h)
    /// Se considera:
    /// - las superficies opacos en contacto con el aire exterior
    /// - las permeabilidad al aire de opacos en función de si es nuevo (o permeab. mejorada) o existente
    /// - los huecos de las superficies opacas anteriores
    /// - la permeabilidad al aire de huecos definida en su construcción
    /// - el volumen interior de la envolvente térmica ()
    /// Se ignoran los huecos sin construcción definida y los muros sin espacio definido
    pub fn n50_he2019(&self) -> N50HEDetail {
        let vol: f32 = self.vol_env_net();
        if vol <= 0.01 {
            info!(
                "n_50=0.00 1/h, Σ(A_o.C_o)=- m³/h, Σ(A_h.C_h)=- m³/h, vol={:.2} m³",
                vol
            );
            return N50HEDetail {
                n50: 0.0,
                walls_c_a: 0.0,
                windows_c_a: 0.0,
                vol,
            };
        };
        let c_o = self.C_o_he2019();
        let (walls_c_a, windows_c_a) = self
            .walls_of_envelope()
            .filter(|w| w.bounds == BoundaryType::EXTERIOR)
            .map(|w| {
                let multiplier = self.get_wallspace(&w).map(|s| s.multiplier).unwrap_or(1.0);
                let wall_ah_ch: f32 = self
                    .windows_of_wall(&w.id)
                    .filter_map(|win| {
                        self.get_wincons(&win)
                            .map(|wincons| Some(win.area * wincons.infcoeff_100))?
                    })
                    .sum();
                (w.area * c_o * multiplier, wall_ah_ch * multiplier)
            })
            .fold((0.0, 0.0), |(acc_ao_co, acc_ah_ch), (e_ao_co, e_ah_ch)| {
                (acc_ao_co + e_ao_co, acc_ah_ch + e_ah_ch)
            });
        let n50 = 0.629 * (walls_c_a + windows_c_a) / vol;
        info!(
            "n_50={:.2} 1/h, Σ(A_o.C_o)={:.2} m³/h, Σ(A_h.C_h)={:.2} m³/h, vol={:.2} m³",
            n50, walls_c_a, windows_c_a, vol
        );
        N50HEDetail {
            n50,
            walls_c_a,
            windows_c_a,
            vol,
        }
    }

    /// Calcula la permeabilidad de opacos a partir de un ensayo de puerta soplante
    /// Se ignoran los huecos sin construcción definida y los muros sin espacio definido
    pub fn wall_inf_100_from_n50(&self, n50: f32) -> f32 {
        let vol: f32 = self.vol_env_net();
        let (sum_wall_area, sum_axc_h): (f32, f32) = self
            .walls_of_envelope()
            .filter(|w| w.bounds == BoundaryType::EXTERIOR)
            .map(|w| {
                let axc_h: f32 = self
                    .windows_of_wall(&w.id)
                    .filter_map(|win| {
                        self.get_wincons(&win)
                            .map(|wincons| Some(win.area * wincons.infcoeff_100))?
                    })
                    .sum();
                let multiplier = self.get_wallspace(&w).map(|s| s.multiplier).unwrap_or(1.0);
                (w.area * multiplier, axc_h * multiplier)
            })
            .fold(
                (0.0, 0.0),
                |(acc_wall_area, acc_axc_h), (e_wall_area, e_axc_h)| {
                    (acc_wall_area + e_wall_area, acc_axc_h + e_axc_h)
                },
            );
        let C_o = ((n50 * vol) / 0.629 - sum_axc_h) / sum_wall_area;
        info!(
            "C_o={:.2}, n_50={:.2}, vol={:.2}, (A_h.C_h)={:.2}, A_o={:.2}",
            C_o, n50, vol, sum_axc_h, sum_wall_area
        );
        C_o
    }

    /// Calcula la transmitancia térmica global K (W/m2K)
    /// Transmitancia media de los elementos en contacto con el aire exterior o con el terreno
    /// Incluye los puentes térmicos
    /// Se ignoran los huecos y muros para los que no está definida su construcción, transmitancia o espacio
    pub fn K_he2019(&self) -> KDetail {
        let (walls_a_u, walls_a, windows_a_u, windows_a): (f32, f32, f32, f32) = self
            .walls_of_envelope()
            .filter_map(|wall| {
                let (win_w_a_u, win_w_a) = self
                    .windows_of_wall(&wall.id)
                    .filter_map(|win_i| {
                        self.get_wincons(&win_i)
                            .map(|wincons| Some((win_i.area * wincons.u, win_i.area)))?
                    })
                    .fold((0.0, 0.0), |(acc_a_u, acc_a), (win_i_a_u, win_i_a)| {
                        (acc_a_u + win_i_a_u, acc_a + win_i_a)
                    });
                let multiplier = self
                    .get_wallspace(&wall)
                    .map(|s| s.multiplier)
                    .unwrap_or(1.0);
                let wall_u = self.u_for_wall(&wall)?;
                Some((
                    wall_u * wall.area * multiplier,
                    wall.area * multiplier,
                    win_w_a_u * multiplier,
                    win_w_a * multiplier,
                ))
            })
            .fold(
                (0.0, 0.0, 0.0, 0.0),
                |(acc_wall_a_u, acc_wall_a, acc_win_a_u, acc_win_a),
                 (wall_a_u, wall_a, win_a_u, win_a)| {
                    (
                        acc_wall_a_u + wall_a_u,
                        acc_wall_a + wall_a,
                        acc_win_a_u + win_a_u,
                        acc_win_a + win_a,
                    )
                },
            );
        let (thermal_bridges_l, thermal_bridges_psi_l): (f32, f32) = self
            .thermal_bridges
            .iter()
            .map(|tb| (tb.l, tb.psi * tb.l))
            .fold((0.0, 0.0), |(acc_l, acc_psi_l), (e_l, e_psi_l)| {
                (acc_l + e_l, acc_psi_l + e_psi_l)
            });

        let total_a_u = walls_a_u + windows_a_u + thermal_bridges_psi_l;
        let total_a = walls_a + windows_a;

        let K = if total_a <= 0.01 {
            0.0
        } else {
            total_a_u / total_a
        };
        info!(
            "K={:.2} W/m²K, A_o={:.2} m², (A.U)_o={:.2} W/K, A_h={:.2} m², (A.U)_h={:.2} W/K, L_pt={:.2} m, Psi.L_pt={:.2} W/K",
            K, walls_a, walls_a_u, windows_a, windows_a_u, thermal_bridges_l, thermal_bridges_psi_l
        );

        KDetail {
            K,
            walls_a,
            walls_a_u,
            windows_a,
            windows_a_u,
            thermal_bridges_l,
            thermal_bridges_psi_l,
        }
    }

    /// Calcula el parámetro de control solar (q_sol;jul) a partir de los datos de radiación total acumulada en julio
    /// Los huecos para los que no está definido su opaco o su construcción no se consideran en el cálculo
    pub fn q_soljul(&self, totradjul: &HashMap<Orientation, f32>) -> f32 {
        let Q_soljul = self
            .windows_of_envelope()
            .filter_map(|w| {
                let wall = self.get_windowwall(&w)?;
                let wincons = self.get_wincons(&w)?;
                let orientation = Orientation::from(wall);
                let radjul = totradjul.get(&orientation).unwrap();
                debug!(
                    "qsoljul de {}: A {:.2}, orient {}, ff {:.2}, gglshwi {:.2}, fshobst {:.2}, H_sol;jul {:.2}",
                    w.name, w.area, orientation, wincons.ff, wincons.gglshwi, w.fshobst, radjul
                );
                Some(w.fshobst * wincons.gglshwi * (1.0 - wincons.ff) * w.area * radjul)
            })
            .sum::<f32>();
        let a_ref = self.a_ref();
        let qsoljul = Q_soljul / a_ref;
        info!(
            "q_sol;jul={:.2} kWh/m².mes, Q_soljul={:.2} kWh/mes, A_ref={:.2}",
            qsoljul, Q_soljul, a_ref
        );
        qsoljul
    }

    /// Transmitancia térmica de una composición de cerramiento, en una posición dada, en W/m2K
    /// Tiene en cuenta la posición del elemento para fijar las resistencias superficiales
    /// Notas:
    /// - en particiones interiores NO se considera el factor b, reductor de temperatura
    /// - NO se ha implementado el cálculo de elementos en contacto con espacios no habitables
    /// - NO se ha implementado el cálculo de cerramientos en contacto con el terreno
    ///     - en HULC los valores por defecto de Ra y D se indican en las opciones generales de
    ///       las construcciones por defecto
    /// - los elementos adiabáticos se reportan con valor 0.0
    /// - los elementos mal definidos (muros sin construcción o sin espacio asignado) se reportan con valor 0.0
    pub fn u_for_wall(&self, wall: &Wall) -> Option<f32> {
        use {BoundaryType::*, SpaceType::*, Tilt::*};

        let position = Tilt::from(wall);
        let bounds: BoundaryType = wall.bounds.into();
        let R_n_perim_ins = self.meta.rn_perim_insulation;
        let D_perim_ins = self.meta.d_perim_insulation;

        let cons = self.get_wallcons(&wall)?;
        let R_intrinsic = cons.r_intrinsic;

        match (bounds, position) {
            // Elementos adiabáticos -----------------------------
            (ADIABATIC, _) => {
                let U = 0.0;
                debug!("{} (adiabático) U={:.2}", wall.name, U);
                Some(U)
            }
            // Elementos en contacto con el exterior -------------
            (EXTERIOR, BOTTOM) => {
                let U = 1.0 / (R_intrinsic + RSI_DESCENDENTE + RSE);
                debug!("{} (suelo) U={:.2}", wall.name, U);
                Some(U)
            }
            (EXTERIOR, TOP) => {
                let U = 1.0 / (R_intrinsic + RSI_ASCENDENTE + RSE);
                debug!("{} (cubierta) U={:.2}", wall.name, U);
                Some(U)
            }
            (EXTERIOR, SIDE) => {
                let U = 1.0 / (R_intrinsic + RSI_HORIZONTAL + RSE);
                debug!("{} (muro) U={:.2}", wall.name, U);
                Some(U)
            }
            // Elementos enterrados ------------------------------
            (GROUND, BOTTOM) => {
                // 1. Solera sobre el terreno: UNE-EN ISO 13370:2010 Apartado 9.1 y 9.3.2
                // Simplificaciones:
                // - forma cuadrada para calcular el perímetro
                // - ancho de muros externos w = 0.3m
                // - lambda de aislamiento = 0,035 W/mK
                //
                // HULC parece estar calculando algo más parecido al método de Winkelman o:
                // let u = 1.0 / (r_intrinsic + RSI_DESCENDENTE + RSE + 0.25 / LAMBDA_GND + 0.01 / LAMBDA_INS);

                // Dimensión característica del suelo (B'). Ver UNE-EN ISO 13370:2010 8.1
                // Calculamos la dimensión característica del **espacio** en el que sitúa el suelo
                // Si este espacio no define el perímetro, lo calculamos suponiendo una superficie cuadrada
                let wspace = self.get_wallspace(&wall)?;
                let gnd_A = wspace.area;
                let gnd_P = wspace
                    .exposed_perimeter
                    .unwrap_or_else(|| 4.0 * f32::sqrt(gnd_A));

                // Soleras sin contacto perimetral con el exterior B' -> inf -> U -> 0
                if gnd_P.abs() < 0.001 {
                    warn!(
                        "{} (solera con perímetro expuesto nulo o casi nulo {:.2}. U = 0.00)",
                        wall.name, gnd_P,
                    );
                    return Some(0.0);
                };

                let B_1 = gnd_A / (0.5 * gnd_P);

                let z = if wspace.z < 0.0 { -wspace.z } else { 0.0 };
                const W: f32 = 0.3; // Simplificación: espesor supuesto de los muros perimetrales
                let d_t = W + LAMBDA_GND * (RSI_DESCENDENTE + R_intrinsic + RSE);

                let U_bf = if (d_t + 0.5 * z) < B_1 {
                    // Soleras sin aislar y moderadamente aisladas
                    (2.0 * LAMBDA_GND / (PI * B_1 + d_t + 0.5 * z))
                        * f32::ln(1.0 + PI * B_1 / (d_t + 0.5 * z))
                } else {
                    // Soleras bien aisladas
                    LAMBDA_GND / (0.457 * B_1 + d_t + 0.5 * z)
                };

                // Efecto del aislamiento perimetral 13770 Anexo B.
                // Espesor aislamiento perimetral d_n = r_n_perim_ins * lambda_ins
                // Espesor equivalente adicional resultante del aislamiento perimetral (d')
                let D_1 = R_n_perim_ins * (LAMBDA_GND - LAMBDA_INS);
                let psi_ge = -LAMBDA_GND / PI
                    * (f32::ln(D_perim_ins / d_t + 1.0) - f32::ln(1.0 + D_perim_ins / (d_t + D_1)));

                let U = U_bf + 2.0 * psi_ge / B_1; // H_g sería U * A

                debug!(
                    "{} (suelo de sótano) U={:.2} (R_n={:.2}, D={:.2}, A={:.2}, P={:.2}, B'={:.2}, z={:.2}, d_t={:.2}, R_f={:.3}, U_bf={:.2}, psi_ge = {:.3})",
                    wall.name,
                    U,
                    R_n_perim_ins,
                    D_perim_ins,
                    gnd_A,
                    gnd_P,
                    B_1,
                    z,
                    d_t,
                    R_intrinsic,
                    U_bf,
                    psi_ge
                );
                Some(U)
            }
            (GROUND, SIDE) => {
                // 2. Muros enterrados UNE-EN ISO 13370:2010 9.3.3
                let U_w = 1.0 / (RSI_HORIZONTAL + R_intrinsic + RSE);
                let space = self.get_wallspace(&wall)?;
                let z = if space.z < 0.0 { -space.z } else { 0.0 };
                // Muros que realmente no son enterrados
                if z.abs() < 0.01 {
                    warn!(
                        "{} (muro de sótano no enterrado z=0) U_w={:.2} (z={:.2})",
                        wall.name, U_w, z,
                    );
                    return Some(U_w);
                };

                // Dimensión característica del suelo del sótano.
                // Suponemos espesor de muros de sótano = 0.30m para cálculo de soleras
                // Usamos el promedio de los suelos del espacio
                let mut d_t = self
                    .walls_of_space(&space.id)
                    .filter(|w| Tilt::from(*w) == BOTTOM)
                    .zip(1..)
                    .fold(0.0, |mean, (w, i)| {
                        // Si no está definida la construcción no participa de la envolvente
                        self.get_wallcons(&w)
                            .map(|wallcons| {
                                (W + LAMBDA_GND * (RSI_DESCENDENTE + wallcons.r_intrinsic + RSE)
                                    + mean * (i - 1) as f32)
                                    / i as f32
                            })
                            .unwrap_or(0.0)
                    });
                const W: f32 = 0.3;

                // Espesor equivalente de los muros de sótano (13)
                let d_w = LAMBDA_GND * (RSI_HORIZONTAL + R_intrinsic + RSE);

                if d_w < d_t {
                    d_t = d_w
                };

                // U del muro completamente enterrado a profundidad z (14)
                let U_bw = if z != 0.0 {
                    (2.0 * LAMBDA_GND / (PI * z))
                        * (1.0 + 0.5 * d_t / (d_t + z))
                        * f32::ln(z / d_w + 1.0)
                } else {
                    U_w
                };

                // Altura neta
                let height_net = space.height - self.top_wall_thickness(&space.id);

                // Altura sobre el terreno (muro no enterrado)
                let h = if height_net > z { height_net - z } else { 0.0 };

                // Si el muro no es enterrado en toda su altura ponderamos U por altura
                let U = if h == 0.0 {
                    // Muro completamente enterrado
                    U_bw
                } else {
                    // Muro con z parcialmente enterrado
                    (z * U_bw + h * U_w) / height_net
                };

                debug!(
                    "{} (muro enterrado) U={:.2} (z={:.2}, h={:.2}, U_w={:.2}, U_bw={:.2}, d_t={:.2}, d_w={:.2})",
                    wall.name, U, z, h, U_w, U_bw, d_t, d_w,
                );
                Some(U)
            }
            // Cubiertas enterradas: el terreno debe estar definido como una capa de tierra con lambda = 2 W/K
            (GROUND, TOP) => {
                let U = 1.0 / (R_intrinsic + RSI_ASCENDENTE + RSE);
                debug!(
                    "{} (cubierta enterrada) U={:.2} (R_f={:.3})",
                    wall.name, U, R_intrinsic
                );
                Some(U)
            }
            // Elementos en contacto con otros espacios ---------------------
            (INTERIOR, position) => {
                // Dos casos:
                // - Suelos en contacto con sótanos no acondicionados / no habitables en contacto con el terreno - ISO 13370:2010 (9.4)
                // - Elementos en contacto con espacios no acondicionados / no habitables - UNE-EN ISO 6946:2007 (5.4.3)
                let space = self.get_wallspace(&wall)?;
                let nextto = match wall.nextto.as_ref() {
                    Some(s) => s,
                    _ => {
                        warn!(
                            "Muro {} ({}) sin definición de espacio adyacente",
                            wall.id, wall.name
                        );
                        return None;
                    }
                };

                let nextspace = match self.get_space(nextto.as_str()) {
                    Some(s) => s,
                    _ => {
                        warn!(
                            "Muro {} ({}) con definición de espacio adyacente incorrecta {}",
                            wall.id,
                            wall.name,
                            nextto.as_str()
                        );
                        return None;
                    }
                };

                let nexttype = nextspace.space_type;

                let posname = match position {
                    BOTTOM => "suelo",
                    TOP => "techo",
                    SIDE => "muro",
                };

                if nexttype == CONDITIONED && space.space_type == CONDITIONED {
                    // Elemento interior con otro espacio acondicionado
                    // HULC no diferencia entre RS según posiciones para elementos interiores
                    let U = 1.0 / (R_intrinsic + 2.0 * RSI_HORIZONTAL);
                    debug!(
                        "{} ({} acondicionado-acondicionado) U_int={:.2}",
                        wall.name, posname, U
                    );
                    Some(U)
                } else {
                    // Comunica un espacio acondicionado con otro no acondicionado

                    // Localizamos el espacio no acondicionado
                    let (uncondspace, thiscondspace) = if nexttype == CONDITIONED {
                        (space, false)
                    } else {
                        (nextspace, true)
                    };

                    // Resistencia del elemento teniendo en cuenta el flujo de calor (UNE-EN ISO 13789 Tabla 8)
                    let R_f = match (position, thiscondspace) {
                        // Suelo de espacio acondicionado hacia no acondicionado inferior
                        // Techo de espacio no acondicionado hacia acondicionado inferior
                        (BOTTOM, true) | (TOP, false) => R_intrinsic + 2.0 * RSI_DESCENDENTE,
                        // Techo de espacio acondicionado hacia no acondicionado superior
                        // Suelo de espacio no acondicionado hacia acondicionado superior
                        (TOP, true) | (BOTTOM, false) => R_intrinsic + 2.0 * RSI_ASCENDENTE,
                        // Muro
                        (SIDE, _) => R_intrinsic + 2.0 * RSI_HORIZONTAL,
                    };

                    // Intercambio de aire en el espacio no acondicionado (¿o podría ser el actual si es el no acondicionado?)
                    let uncondspace_v = (uncondspace.height
                        - self.top_wall_thickness(&uncondspace.id))
                        * uncondspace.area;
                    let n_ven = match uncondspace.n_v {
                        Some(n_v) => n_v,
                        _ => match self.meta.global_ventilation_l_s {
                            Some(global_ventilation) => {
                                3.6 * global_ventilation / self.vol_env_inh_net()
                            }
                            _ => {
                                // Espacio mal definido (ni tiene n_v ni hay definición global de ventilación)
                                warn!("Definición global (l/s) no definida para espacio no acondicionado sin n_v {} ({})", uncondspace.id, uncondspace.name);
                                0.0
                            }
                        },
                    };

                    // CASO: interior en contacto con sótano no calefactado - ISO 13370:2010 (9.4)
                    // CASO: interior en contacto con otro espacio no habitable / no acondicionado - UNE-EN ISO 6946:2007 (5.4.3)
                    // Calculamos el A.U de los elementos del espacio que dan al exterior o al terreno (excluye interiores))
                    // Como hemos asignado U_bw y U_bf a los muros y suelos en contacto con el terreno, ya se tiene en cuenta
                    // la parte enterrada correctamente (fracción enterrada y superficie expuesta, ya que no se consideran los que dan a interiores)
                    let UA_e_k = self
                        .walls_of_space(&uncondspace.id)
                        .filter(|wall| wall.bounds == GROUND || wall.bounds == EXTERIOR)
                        .filter_map(|wall| {
                            // A·U de muros (y suelos) + A.U de sus huecos
                            let wall_u = self.u_for_wall(wall)?;
                            let win_axu = self
                                .windows_of_wall(&wall.id)
                                .filter_map(|win| {
                                    self.get_wincons(&win)
                                        // Si no está definida la construcción no participa de la envolvente
                                        .map(|wincons| Some(win.area * wincons.u))?
                                })
                                .sum::<f32>();
                            Some(wall.area * wall_u + win_axu)
                        })
                        .sum::<f32>();
                    // 1/U = 1/U_f + A_i / (sum_k(A_e_k·U_e_k) + 0.33·n·V) (17)
                    // En la fórmula anterior, para espacios no acondicionados, se indica que se excluyen suelos, pero no entiendo bien por qué.
                    // Esta fórmula, cuando los A_e_k y U_e_k incluyen los muros y suelos con el terreno U_bw y U_bf, con la parte proporcional de
                    // muros al exterior, es equivalente a la que indica la 13370
                    let A_i = wall.area;
                    let H_ue = UA_e_k + 0.33 * n_ven * uncondspace_v;
                    let R_u = A_i / H_ue;
                    let U = 1.0 / (R_f + R_u);

                    debug!(
                            "{} ({} acondicionado-no acondicionado/sotano) U={:.2} (R_f={:.3}, R_u={:.3}, A_i={:.2}, U_f=1/R_f={:.2}",
                            wall.name, posname, U, R_f, R_u, A_i, 1.0/R_f
                        );
                    Some(U)
                }
            }
        }
    }

    /// Elemento opaco de techo de un espacio
    fn top_wall_of_space<'a>(&'a self, spaceid: &'a str) -> Option<&'a Wall> {
        self.walls.iter().find(move |w| {
            match w.tilt.into() {
                // Muros exteriores o cubiertas sobre el espacio
                Tilt::TOP => &w.space == &spaceid,
                // Es un cerramiento interior sobre este espacio
                Tilt::BOTTOM => w.nextto.as_ref().map(|s| s == &spaceid).unwrap_or(false),
                _ => false,
            }
        })
    }

    /// Grosor de un elemento opaco
    fn wall_thickness(&self, wallid: &str) -> f32 {
        self.get_wall(wallid)
            .and_then(|w| self.get_wallcons(&w).map(|c| c.thickness))
            .unwrap_or(0.0)
    }

    /// Grosor del forjado superior de un espacio
    /// TODO: la altura neta debería calcularse promediando los grosores de todos los muros que cierren el espacio,
    /// TODO: estos podrían ser más de uno pero este cálculo ahora se hace con el primero que se localiza
    fn top_wall_thickness(&self, spaceid: &str) -> f32 {
        self.top_wall_of_space(&spaceid)
            .map(|w| self.wall_thickness(&w.id))
            .unwrap_or(0.0)
    }

    /// Comprueba consistencia del modelo y devuelve lista de avisos / errores detectados
    ///
    /// 1. Elementos mal definidos que se ignorarán en el cálculo:
    ///     - Huecos sin referencias de construcciones válidas
    ///     - Huecos sin referencias de muros válidas
    ///     - Muros sin referencias de espacios válidas
    ///     - Muros sin referencias de construcciones válidas
    ///     - Muros con nextto con referencia no válida
    pub fn check_model(&self) -> Vec<Warning> {
        use WarningLevel::WARNING;

        let spaceids: HashSet<&str> = self.spaces.iter().map(|s| s.id.as_str()).collect();
        let wallids: HashSet<&str> = self.walls.iter().map(|w| w.id.as_str()).collect();
        let wallconsids: HashSet<&str> = self.wallcons.iter().map(|c| c.id.as_str()).collect();
        let winconsids: HashSet<&str> = self.wincons.iter().map(|c| c.id.as_str()).collect();

        let mut warnings = Vec::new();

        self.walls.iter().for_each(|w| {
            if !spaceids.contains(w.space.as_str()) {
                warnings.push(Warning {
                    level: WARNING,
                    id: Some(w.id.clone()),
                    msg: format!(
                        "Muro {} ({}) con referencia incorrecta de espacio {}",
                        w.id, w.name, w.space
                    ),
                })
            };
            if !wallconsids.contains(w.cons.as_str()) {
                warnings.push(Warning {
                    level: WARNING,
                    id: Some(w.id.clone()),
                    msg: format!(
                        "Muro {} ({}) con referencia incorrecta de construcción {}",
                        w.id, w.name, w.cons
                    ),
                })
            };
            if w.nextto.is_some() && !spaceids.contains(w.nextto.clone().unwrap().as_str()) {
                warnings.push(Warning {
                    level: WARNING,
                    id: Some(w.id.clone()),
                    msg: format!(
                        "Muro {} ({}) con referencia incorrecta de espacio adyacente {}",
                        w.id,
                        w.name,
                        w.nextto.clone().unwrap()
                    ),
                })
            };
        });

        self.windows.iter().for_each(|w| {
            if !wallids.contains(w.wall.as_str()) {
                warnings.push(Warning {
                    level: WARNING,
                    id: Some(w.id.clone()),
                    msg: format!(
                        "Hueco {} ({}) con referencia incorrecta de opaco {}",
                        w.id, w.name, w.wall
                    ),
                })
            };
            if !winconsids.contains(w.cons.as_str()) {
                warnings.push(Warning {
                    level: WARNING,
                    id: Some(w.id.clone()),
                    msg: format!(
                        "Hueco {} ({}) con referencia incorrecta de construcción {}",
                        w.id, w.name, w.cons
                    ),
                })
            };
        });
        warnings
    }
}
