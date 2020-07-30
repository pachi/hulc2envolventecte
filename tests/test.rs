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

use hulc2envolventecte::{
    collect_hulc_data,
    parsers::{bdl, ctehexml, kyg, tbl},
    utils,
};
use std::convert::TryFrom;

macro_rules! assert_almost_eq {
    ($a:expr, $b:expr, $c:expr) => {
        if ($a - $b).abs() > $c {
            panic!(format!(
                "assertion failed: `abs(left - right) < {}`, (left: `{}`, right: `{}`)",
                $c, $a, $b
            ));
        }
    };
}

#[test]
fn test_polygon() {
    use bdl::{BdlBlock, Polygon};
    let polblk: BdlBlock =
        r#"\"P01_E01_Pol2\" = POLYGON                                             
    V1   =( 14.97, 11.39 )
    V2   =( 10.84, 11.39 )
    V3   =( 10.86, 0 )
    V4   =( 18.22, 0 )
    V5   =( 18.22, 9.04 )
    V6   =( 14.97, 9.04 )
    .."#
        .parse()
        .unwrap();
    let pol: Polygon = Polygon::try_from(polblk).unwrap();
    assert_eq!(pol.area(), 76.306793);
    assert_eq!(pol.edge_indices("V1").unwrap(), [0, 1]);
    assert_eq!(pol.edge_indices("V6").unwrap(), [5, 0]);
    assert_eq!(pol.edge_length("V3"), 18.22 - 10.86);
}

#[test]
fn test_polygon2() {
    use bdl::{BdlBlock, Polygon};
    let polblk: BdlBlock =
        r#"\"TEST_POLYGON\" = POLYGON                                             
    V1   =( 1, 1 )
    V2   =( 2, 1 )
    V3   =( 3, 2 )
    V4   =( 3, 3 )
    V5   =( 1, 3 )
    V6   =( 0, 2 )
    .."#
        .parse()
        .unwrap();
    let pol: Polygon = Polygon::try_from(polblk).unwrap();
    assert_eq!(pol.area(), 4.5);
    assert_eq!(pol.perimeter(), 8.2426405);
    assert_eq!(pol.edge_indices("V1").unwrap(), [0, 1]);
    assert_eq!(pol.edge_indices("V6").unwrap(), [5, 0]);
    assert_eq!(pol.edge_length("V3"), 1.0);
    // lado horizontal hacia la derecha
    assert_eq!(pol.edge_orient("V1", 0.0), 0.0);
    // lado inclinado 45º hacia la derecha-arriba
    assert_eq!(pol.edge_orient("V2", 0.0), 45.0);
    // lado vertical hacia arriba
    assert_eq!(pol.edge_orient("V3", 0.0), 90.0);
    // lado horizontal hacia la izquierda
    assert_eq!(pol.edge_orient("V4", 0.0), 180.0);
    // lado inclinado 45º hacia la izquierda-abajo
    assert_eq!(pol.edge_orient("V5", 0.0), 225.0);
    // lado inclinado 45º hacia la derecha-abajo
    assert_eq!(pol.edge_orient("V6", 0.0), 315.0);
    // V1 con norte desviado 45º
    assert_eq!(pol.edge_orient("V1", 45.0), 315.0);
    // V5 con norte desviado 45º
    assert_eq!(pol.edge_orient("V5", 45.0), 180.0);
    // V2 con norte desviado 45º
    assert_eq!(pol.edge_orient("V2", 45.0), 0.0);
}

#[test]
fn test_test_spaces_caso_a() {
    let tbl = tbl::parse("tests/casoA/NewBDL_O.tbl").unwrap();
    let ctehexmlpath = ctehexml::find_ctehexml("tests/casoA").unwrap();
    let xmldata = ctehexml::parse(&ctehexmlpath.unwrap()).unwrap();
    let bdl = xmldata.bdldata;

    for (name, space) in tbl.spaces {
        let spc = bdl.get_space(&name).unwrap();
        let poly = &spc.polygon;
        assert_eq!(space.area, poly.area())
    }
}

#[test]
fn test_bdl_parse() {
    let _data = ctehexml::parse("tests/00_plurif_s3_v0_d3/00_plurif_s3_v0_d3.ctehexml").unwrap();
    let _data = ctehexml::parse("tests/casoA/casoa.ctehexml").unwrap();
    let data = ctehexml::parse("tests/casoC/casoc.ctehexml").unwrap();
    let bdldb = &data.bdldata;

    #[allow(unused_variables)]
    let bdl::Data {
        meta,
        db,
        spaces,
        walls,
        windows,
        tbridges,
        shadings,
        spaceconds,
        systemconds,
        schedules,
    } = bdldb;
    // println!("{:#?}", db);
    // println!("{:#?}", spaces);
    // println!("{:#?}", walls);
    // println!("{:#?}", shadings);

    // Cálculos básicos sobre elementos de la envolvente

    // TODO: Hacer más casos de orientación respecto al sur, con muros definidos con AZIMUTH
    // ya que ahora mismo solo se prueban los definidos por vértices y no está claro
    // si los valores que se obtienen en ese parámetro son respecto al norte (los de espacios sí)

    // Espacio
    let s = bdldb.get_space("P02_E01").unwrap();
    assert_eq!(s.height, 3.0); // Altura
    assert_eq!(s.space_height(bdldb).unwrap(), 2.62); // Altura libre
    assert_eq!(s.area(), 150.0); // Área 10m x 15m
    assert_eq!(s.perimeter(), 50.0); // Perímetro (10 + 15) x 2

    // Forjado interior
    let w = bdldb.get_wall("P02_E01_FI001").unwrap();
    assert_eq!(w.gross_area(bdldb).unwrap(), 49.985004);
    assert_eq!(w.net_area(bdldb).unwrap(), 49.985004);
    assert_eq!(w.space, "P02_E01");
    assert_eq!(w.tilt, 180.0);
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 0.0); // Horizontal

    // Solera
    let w = bdldb.get_wall("P01_E01_FTER001").unwrap();
    assert_eq!(w.gross_area(bdldb).unwrap(), 50.0);
    assert_eq!(w.net_area(bdldb).unwrap(), 50.0);
    assert_eq!(w.space, "P01_E01");
    assert_eq!(w.tilt, 180.0);
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 0.0); // Horizontal

    // Pared exterior
    let w = bdldb.get_wall("P01_E01_PE003").unwrap();
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 0.0); // Norte
    let w = bdldb.get_wall("P04_E01_ME001").unwrap();
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 0.0); // Norte
    assert_eq!(w.gross_area(bdldb).unwrap(), 17.5);

    // Muro exterior
    let w = bdldb.get_wall("P01_E01_PE001").unwrap();
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 180.0); // Sur

    // Muro exterior
    let w = bdldb.get_wall("P02_E01_PE003").unwrap();
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 90.0); // Este

    // Muro interior
    let w = bdldb.get_wall("P02_E01_PE001").unwrap();
    assert_eq!(w.gross_area(bdldb).unwrap(), 30.0);
    assert_eq!(w.net_area(bdldb).unwrap(), 28.0);
    assert_eq!(w.space, "P02_E01");
    assert_eq!(w.tilt, 90.0);
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 270.0); // Oeste

    let v = bdldb.get_window("P02_E01_PE001_V").unwrap();
    assert_eq!(v.area(), 2.0);
    assert_eq!(v.wall, "P02_E01_PE001");
    assert_eq!(v.tilt(bdldb).unwrap(), 90.0);
    assert_eq!(v.azimuth(0.0, bdldb).unwrap(), 270.0); // Oeste

    // Cubiertas
    let w = bdldb.get_wall("P03_E01_CUB001").unwrap();
    assert_almost_eq!(w.gross_area(bdldb).unwrap(), 50.0, 0.005);
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 0.0); // Horizontal
    assert_eq!(w.tilt, 0.0); // Horizontal
    let w = bdldb.get_wall("P04_E01_CUB001").unwrap();
    assert_almost_eq!(w.gross_area(bdldb).unwrap(), 50.99020, 0.005);
    assert_eq!(w.azimuth(0.0, bdldb).unwrap(), 90.0); // Este
    assert_eq!(w.tilt, 11.30993);

    // // Volumen acondicionado de la envolvente:
    // // - volumen de los espacios acondicionados
    // // - restamos volumen de los forjados interiores y de las cubiertas
    // let mut v = 0.0;
    // for spc in &bdldb.spaces {
    //     if spc.stype == "CONDITIONED" {
    //         v += spc.space_height(bdldb).unwrap() * spc.area(bdldb).unwrap();
    //     }
    // }
    // assert_almost_eq!(v, 1055.949951, 0.005);
}

#[test]
fn test_caso_a() {
    let ctehexmlpath = ctehexml::find_ctehexml("tests/casoA").unwrap();
    let kygpath = kyg::find_kyg("tests/casoA").unwrap();
    let tblpath = tbl::find_tbl("tests/casoA").unwrap();
    let data = collect_hulc_data(ctehexmlpath, kygpath, tblpath).unwrap();
    assert_eq!(data.a_util_ref(), 400.0);
    assert_eq!(data.meta.climate, "D3");
    assert_eq!(data.windows.len(), 10);
    assert_eq!(data.walls.len(), 35); // 19 en ET
    assert_eq!(data.thermal_bridges.len(), 11); // 7 en kyg
    let results: Vec<&str> = vec![
        "P02_E01_FI001",
        "P02_E01_FI002",
        "P02_E01_ME001",
        "P02_E01_MED001",
        "P02_E01_PE001",
        "P02_E01_PE002",
        "P02_E01_PE003",
        "P03_E01_FI003",
    ];
    assert_eq!(
        data.get_space_walls("P02_E01")
            .iter()
            .map(|w| w.name.as_str())
            .collect::<Vec<_>>(),
        results
    );
}

#[test]
fn test_caso_c() {
    let ctehexmlpath = ctehexml::find_ctehexml("tests/casoC").unwrap();
    let kygpath = kyg::find_kyg("tests/casoC").unwrap();
    let tblpath = tbl::find_tbl("tests/casoC").unwrap();
    let data = collect_hulc_data(ctehexmlpath, kygpath, tblpath).unwrap();
    assert_eq!(data.a_util_ref(), 400.0);
    assert_eq!(data.meta.climate, "D3");
    assert_eq!(data.windows.len(), 9);
    assert_eq!(data.walls.len(), 33); // 27 en ET
    assert_eq!(data.thermal_bridges.len(), 11); // 7 en kyg
}

// Caso más antiguo con archivo generado con el HULC2018 que salió a información pública
#[test]
fn parse_test_data() {
    let ctehexmlpath = ctehexml::find_ctehexml("tests/data").unwrap();
    let kygpath = kyg::find_kyg("tests/data").unwrap();
    let tblpath = tbl::find_tbl("tests/data").unwrap();
    let data = collect_hulc_data(ctehexmlpath, kygpath, tblpath).unwrap();
    assert_eq!(data.a_util_ref(), 1673.92);
    assert_eq!(data.meta.climate, "D3");
    assert_eq!(data.windows.len(), 92);
    assert_eq!(data.walls.len(), 127); // 68 en ET
    assert_eq!(data.thermal_bridges.len(), 11); // 6 en kyg
}

#[test]
fn parse_test_data2() {
    let ctehexmlpath = ctehexml::find_ctehexml("tests/ejemplopmt_HuecosOK").unwrap();
    let kygpath = kyg::find_kyg("tests/ejemplopmt_HuecosOK").unwrap();
    let tblpath = tbl::find_tbl("tests/ejemplopmt_HuecosOK").unwrap();
    // Las versiones más nuevas usan la coma en KyGananciasSolares.txt como separador decimal
    let data = collect_hulc_data(ctehexmlpath, kygpath, tblpath).unwrap();
    assert_eq!(data.a_util_ref(), 1063.03);
    assert_eq!(data.meta.climate, "B3");
    assert_eq!(data.windows.len(), 29);
    assert_eq!(data.walls.len(), 95); // 60 en ET
    assert_eq!(data.thermal_bridges.len(), 11); // 7 en kyg
}

// #[ignore]
#[test]
fn parse_lider_bdl() {
    let mut count: u32 = 0;
    for ff in std::fs::read_dir("tests/liderdata/").unwrap() {
        let file = ff.unwrap().path().to_str().unwrap().to_string();
        if !file.ends_with(".CTE") && !file.ends_with(".cte") {
            continue;
        };
        println!("Examinando archivo {:#?}", file);
        let strdata = utils::read_latin1_file(&file).unwrap();
        let _data = bdl::Data::new(&strdata).unwrap();
        count += 1;
    }
    println!("Comprobados {} archivos antiguos", count);
}
