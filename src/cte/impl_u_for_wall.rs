// Copyright (c) 2018-2020 Rafael Villar Burke <pachi@ietcc.csic.es>
// Distributed under the MIT License
// (See acoompanying LICENSE file or a copy at http://opensource.org/licenses/MIT)

//! Implementación del cálculo de la U de una composión constructiva de opaco, según su posición
//! - UNE-EN ISO 13789:2010 transmisión general
//! - UNE-EN ISO 6946:2012 para elementos opacos
//! - UNE-EN ISO 13770:2017 para elementos en contacto con el terremo

use std::f32::consts::PI;

use log::info;

pub use super::{BoundaryType, Model, Orientation, SpaceType, Tilt, Wall, WallCons, Window};

// Resistencias superficiales UNE-EN ISO 6946 [m2·K/W]
const RSI_ASCENDENTE: f32 = 0.10;
const RSI_HORIZONTAL: f32 = 0.13;
const RSI_DESCENDENTE: f32 = 0.17;
const RSE: f32 = 0.04;
// conductividad del terreno no helado, en [W/(m·K)]
const LAMBDA_GND: f32 = 2.0;
const LAMBDA_INS: f32 = 0.035;

impl Model {
    /// Iterador de los huecos pertenecientes a un muro
    pub fn windows_of_wall<'a>(&'a self, wallname: &'a str) -> impl Iterator<Item = &'a Window> {
        self.windows.values().filter(move |w| w.wall == wallname)
    }

    /// Iterador de los cerramientos (incluyendo muros, suelos y techos) que delimitan un espacio
    pub fn walls_of_space<'a>(&'a self, space: &'a str) -> impl Iterator<Item = &'a Wall> {
        self.walls.values().filter(move |w| {
            w.space == space
                || (if let Some(ref spc) = w.nextto {
                    spc == space
                } else {
                    false
                })
        })
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
    #[allow(non_snake_case)]
    pub fn u_for_wall(&self, wall: &Wall) -> f32 {
        use {BoundaryType::*, SpaceType::*, Tilt::*};

        let position: Tilt = wall.tilt.into();
        let bounds: BoundaryType = wall.bounds.into();
        let z = wall.zground.unwrap_or(0.0);
        let R_n_perim_ins = self.meta.rn_perim_insulation;
        let D_perim_ins = self.meta.d_perim_insulation;

        let cons = self.wallcons.get(&wall.cons).unwrap();
        let R_intrinsic = cons.r_intrinsic;

        let posname = match position {
            BOTTOM => "suelo",
            TOP => "techo",
            SIDE => "muro",
        };

        match (bounds, position) {
            // Elementos adiabáticos -----------------------------
            (ADIABATIC, _) => {
                let U = 0.0;
                info!("{} (adiabático) U={:.2}", wall.name, U);
                U
            }
            // Elementos en contacto con el exterior -------------
            (EXTERIOR, BOTTOM) => {
                let U = 1.0 / (R_intrinsic + RSI_DESCENDENTE + RSE);
                info!("{} (suelo) U={:.2}", wall.name, U);
                U
            }
            (EXTERIOR, TOP) => {
                let U = 1.0 / (R_intrinsic + RSI_ASCENDENTE + RSE);
                info!("{} (cubierta) U={:.2}", wall.name, U);
                U
            }
            (EXTERIOR, SIDE) => {
                let U = 1.0 / (R_intrinsic + RSI_HORIZONTAL + RSE);
                info!("{} (muro) U={:.2}", wall.name, U);
                U
            }
            // Elementos enterrados ------------------------------
            (UNDERGROUND, BOTTOM) => {
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
                let wspace = self.spaces.get(&wall.space).unwrap();
                let gnd_A = wspace.area;
                let gnd_P = wspace
                    .exposed_perimeter
                    .unwrap_or_else(|| 4.0 * f32::sqrt(gnd_A));
                let B_1 = gnd_A / (0.5 * gnd_P);

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

                info!(
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
                U
            }
            (UNDERGROUND, SIDE) => {
                // 2. Muros enterrados UNE-EN ISO 13370:2010 9.3.3
                let U_w = 1.0 / (RSI_HORIZONTAL + R_intrinsic + RSE);

                // Muros que realmente no son enterrados
                if z.abs() < 0.1 {
                    info!(
                        "{} (muro de sótano no enterrado z=0) U_w={:.2} (z={:.2})",
                        wall.name, U_w, z,
                    );
                    return U_w;
                };

                // Dimensión característica del suelo del sótano.
                // Suponemos espesor de muros de sótano = 0.30m para cálculo de soleras
                // Usamos el promedio de los suelos del espacio
                let space = self.spaces.get(&wall.space).unwrap();
                let mut d_t = self
                    .walls_of_space(&space.name)
                    .filter(|w| Tilt::from(w.tilt) == BOTTOM)
                    .zip(1..)
                    .fold(0.0, |mean, (w, i)| {
                        (W + LAMBDA_GND
                            * (RSI_DESCENDENTE
                                + self.wallcons.get(&w.cons).unwrap().r_intrinsic
                                + RSE)
                            + mean * (i - 1) as f32)
                            / i as f32
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

                // Altura sobre el terreno (muro no enterrado)
                let h = if space.height_net > z {
                    space.height_net - z
                } else {
                    0.0
                };

                // Si el muro no es enterrado en toda su altura ponderamos U por altura
                let U = if h == 0.0 {
                    // Muro completamente enterrado
                    U_bw
                } else {
                    // Muro con z parcialmente enterrado
                    (z * U_bw + h * U_w) / space.height_net
                };

                info!(
                    "{} (muro enterrado) U={:.2} (z={:.2}, h={:.2}, U_w={:.2}, U_bw={:.2}, d_t={:.2}, d_w={:.2})",
                    wall.name, U, z, h, U_w, U_bw, d_t, d_w,
                );
                U
            }
            // Cubiertas enterradas: el terreno debe estar definido como una capa de tierra con lambda = 2 W/K
            (UNDERGROUND, TOP) => {
                let U = 1.0 / (R_intrinsic + RSI_ASCENDENTE + RSE);
                info!(
                    "{} (cubierta enterrada) U={:.2} (R_f={:.3})",
                    wall.name, U, R_intrinsic
                );
                U
            }
            // Elementos en contacto con otros espacios ---------------------
            (INTERIOR, position @ _) => {
                // Dos casos:
                // - Suelos en contacto con sótanos no acondicionados / no habitables en contacto con el terreno - ISO 13370:2010 (9.4)
                // - Elementos en contacto con espacios no acondicionados / no habitables - UNE-EN ISO 6946:2007 (5.4.3)
                let space = self.spaces.get(&wall.space).unwrap();
                let nextto = wall.nextto.as_ref().unwrap();
                let nextspace = self.spaces.get(nextto.as_str()).unwrap();
                let nexttype = nextspace.space_type;

                if nexttype == CONDITIONED && space.space_type == CONDITIONED {
                    // Elemento interior con otro espacio acondicionado
                    // HULC no diferencia entre RS según posiciones para elementos interiores
                    let U = 1.0 / (R_intrinsic + 2.0 * RSI_HORIZONTAL);
                    info!(
                        "{} ({} acondicionado-acondicionado) U_int={:.2}",
                        wall.name, posname, U
                    );
                    U
                } else {
                    // Comunica un espacio acondicionado con otro no acondicionado

                    // Localizamos el espacio no acondicionado
                    let (uncondspace, thiscondspace) = if nexttype == CONDITIONED {
                        (space, false)
                    } else {
                        (nextspace, true)
                    };

                    // Resistencia del elemento teniendo en cuenta el flujo de calor
                    let R_f = match (position, thiscondspace) {
                        // Suelo de espacio acondicionado hacia no acondicionado inferior
                        // Suelo de espacio no acondicionado hacia acondicionado inferior
                        (BOTTOM, true) | (TOP, false) => R_intrinsic + 2.0 * RSI_DESCENDENTE,
                        // Techo de espacio acondicionado hacia no acondicionado superior
                        // Suelo de espacio no acondicionado hacia acondicionado superior
                        (TOP, true) | (BOTTOM, false) => R_intrinsic + 2.0 * RSI_ASCENDENTE,
                        // Muro
                        (SIDE, _) => R_intrinsic + 2.0 * RSI_HORIZONTAL,
                    };

                    // Intercambio de aire en el espacio no acondicionado (¿o podría ser el actual si es el no acondicionado?)
                    let uncondspace_v = uncondspace.height_net * uncondspace.area;
                    let n_ven = match uncondspace.n_v {
                        Some(n_v) => n_v,
                        _ => {
                            3.6 * self.meta.global_ventilation_l_s.unwrap() / self.vol_env_inh_net()
                        }
                    };

                    // CASO: interior en contacto con sótano no calefactado - ISO 13370:2010 (9.4)
                    // CASO: interior en contacto con otro espacio no habitable / no acondicionado - UNE-EN ISO 6946:2007 (5.4.3)
                    // Calculamos el A.U de los elementos del espacio que dan al exterior o al terreno (excluye interiores))
                    // Como hemos asignado U_bw y U_bf a los muros y suelos en contacto con el terreno, ya se tiene en cuenta
                    // la parte enterrada correctamente (fracción enterrada y superficie expuesta, ya que no se consideran los que dan a interiores)
                    let UA_e_k = self
                        .walls_of_space(&uncondspace.name)
                        .filter(|w| w.bounds == UNDERGROUND || w.bounds == EXTERIOR)
                        .map(|w| {
                            // A·U de muros (y suelos) + A.U de sus huecos
                            w.area * self.u_for_wall(w)
                                + self
                                    .windows_of_wall(&w.name)
                                    .map(|win| win.area * self.wincons.get(&win.cons).unwrap().u)
                                    .sum::<f32>()
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

                    info!(
                            "{} ({} acondicionado-no acondicionado/sotano) U={:.2} (R_f={:.3}, R_u={:.3}, A_i={:.2}, U_f=1/R_f={:.2}",
                            wall.name, posname, U, R_f, R_u, A_i, 1.0/R_f
                        );
                    U
                }
            }
        }
    }
}
