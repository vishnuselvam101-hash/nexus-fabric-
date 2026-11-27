//! NFJ-1 S1-L5b: fuzz table for the shape geometry and the single-part front half: every field of every shape gets
//! null-like, zero, negative, tiny, huge and non-finite values. No panic; an accepted request has only finite numbers;
//! a refused one carries a stable code (ZK review_code_fuzz.mjs idea, DESIGN 2.5 item 7).

use nf_core::order::{self, CylParams, PlateParams, RibParams};
use nf_core::shapes::*;

const BAD: [f64; 12] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    0.0,
    -0.0,
    -1.0,
    1e-12,
    5e-324,
    1e8,
    1e8 + 1.0,
    1e300,
    f64::MAX,
];

fn code_known(c: &str) -> bool {
    SHAPE_REFUSAL_CODES.contains(&c)
}

fn finite_all(xs: &[f64]) -> bool {
    xs.iter().all(|x| x.is_finite())
}

#[test]
fn ring_cylinder_cone_fuzz() {
    let ok_ring = RingSpec {
        od: 800.0,
        id: 300.0,
        t: 16.0,
        split: 2,
    };
    for v in BAD {
        for f in 0..3 {
            let mut s = ok_ring;
            match f {
                0 => s.od = v,
                1 => s.id = v,
                _ => s.t = v,
            }
            match ring(s) {
                Ok(r) => assert!(
                    finite_all(&[r.area_total, r.mass_kg])
                        && r.pieces.iter().all(|p| p.area.is_finite()),
                    "{s:?}"
                ),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
        }
        for f in 0..3 {
            let mut s = CylSpec {
                d: 2641.6,
                t: 16.0,
                l: 1500.0,
                seams: 2,
            };
            match f {
                0 => s.d = v,
                1 => s.t = v,
                _ => s.l = v,
            }
            match cylinder(s) {
                Ok(c) => assert!(
                    finite_all(&[c.circ, c.plate_w, c.area_total, c.mass_kg]),
                    "{s:?}"
                ),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
        }
        for f in 0..4 {
            let mut s = ConeSpec {
                d1: 1200.0,
                d2: 800.0,
                l: 700.0,
                t: 12.0,
                basis: Basis::Mid,
                split: 3,
            };
            match f {
                0 => s.d1 = v,
                1 => s.d2 = v,
                2 => s.l = v,
                _ => s.t = v,
            }
            match cone_split(s) {
                Ok(c) => assert!(
                    finite_all(&[c.chord, c.height, c.piece_area, c.mass_kg, c.piece_angle]),
                    "{s:?}"
                ),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
            match ecc_cone_split(s) {
                Ok(c) => assert!(
                    finite_all(&[c.area_total, c.mass_kg])
                        && c.pieces.iter().all(|p| p.dev_angle.is_finite()),
                    "{s:?}"
                ),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
        }
    }
    for n in [0, 13, u32::MAX] {
        assert_eq!(
            ring(RingSpec {
                split: n,
                ..ok_ring
            })
            .unwrap_err()
            .code,
            "shape_split_out_of_range"
        );
        assert_eq!(
            cone_split(ConeSpec {
                d1: 1200.0,
                d2: 800.0,
                l: 700.0,
                t: 12.0,
                basis: Basis::Mid,
                split: n
            })
            .unwrap_err()
            .code,
            "shape_split_out_of_range"
        );
    }
}

#[test]
fn plate_rib_stock_fuzz() {
    for v in BAD {
        for f in 0..9 {
            let mut s = PlateSpec {
                w: 300.0,
                h: 200.0,
                w2: Some(150.0),
                off: Some(10.0),
                t: 9.0,
                c: Some(10.0),
                c_at: 0,
                hole: Some(HoleGrid {
                    d: 22.0,
                    x: 60.0,
                    y: 50.0,
                    nx: 2,
                    ny: 2,
                    px: 100.0,
                    py: 50.0,
                }),
            };
            match f {
                0 => s.w = v,
                1 => s.h = v,
                2 => s.w2 = Some(v),
                3 => s.off = Some(v),
                4 => s.t = v,
                5 => s.c = Some(v),
                6 => s.hole.as_mut().unwrap().d = v,
                7 => s.hole.as_mut().unwrap().x = v,
                _ => s.hole.as_mut().unwrap().px = v,
            }
            match plate(s) {
                Ok(p) => assert!(
                    finite_all(&[p.area, p.perimeter, p.mass_kg, p.w, p.h, p.blank_kg])
                        && p.outline.iter().all(|q| finite_all(q)),
                    "{s:?}"
                ),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
        }
        for f in 0..5 {
            let mut s = RibSpec {
                ro: 159.25,
                vc: 70.0,
                b: 60.0,
                h: 50.0,
                top: 0.0,
            };
            match f {
                0 => s.ro = v,
                1 => s.vc = v,
                2 => s.b = v,
                3 => s.h = v,
                _ => s.top = v,
            }
            match rib(s) {
                Ok(r) => assert!(finite_all(&[r.area, r.x_c, r.u0]), "{s:?}"),
                Err(e) => assert!(code_known(e.code), "{s:?} {e:?}"),
            }
        }
        assert!(fit_stock(v, 1.0, &[Stock { w: 100.0, l: 200.0 }], 0.0).is_none() || v.is_finite());
        let _ = min_cyl_seams(
            v,
            16.0,
            1500.0,
            1,
            &[Stock {
                w: 2438.0,
                l: 6096.0,
            }],
            0.0,
        )
        .map(|r| assert!(r.is_none() || r.is_some_and(|k| (1..=6).contains(&k))))
        .map_err(|e| assert!(code_known(e.code)));
    }
}

#[test]
fn front_half_never_produces_nan_pieces() {
    let stock = [Stock {
        w: 2438.0,
        l: 6096.0,
    }];
    for v in BAD {
        for f in 0..8 {
            let mut p = PlateParams {
                t: Some(9.0),
                plate_w: Some(300.0),
                plate_h: Some(200.0),
                plate_w2: Some(150.0),
                plate_off: Some(0.0),
                plate_c: Some(10.0),
                plate_c_at: Some(0.0),
                hole_d: Some(22.0),
                hole_x: Some(60.0),
                hole_y: Some(50.0),
                ..Default::default()
            };
            match f {
                0 => p.t = Some(v),
                1 => p.plate_w = Some(v),
                2 => p.plate_h = Some(v),
                3 => p.plate_w2 = Some(v),
                4 => p.plate_c = Some(v),
                5 => p.plate_c_at = Some(v),
                6 => p.hole_d = Some(v),
                _ => p.hole_nx = Some(v),
            }
            let r = order::check_plate(&p, None);
            if let Some(pc) = r.piece {
                assert!(finite_all(&[pc.kg, pc.gross_kg, pc.plate.area]), "{p:?}");
            }
        }
        for f in 0..4 {
            let mut c = CylParams {
                d: Some(2641.6),
                t: Some(16.0),
                l: Some(1500.0),
                split: Some(2.0),
            };
            match f {
                0 => c.d = Some(v),
                1 => c.t = Some(v),
                2 => c.l = Some(v),
                _ => c.split = Some(v),
            }
            if let Some(r) = order::check_cylinder(&c, &stock, 0.0) {
                assert!(finite_all(&[r.kg, r.plate_w, r.plate_h, r.circ]), "{c:?}");
            }
        }
        for f in 0..6 {
            let mut r = RibParams {
                t: Some(6.0),
                main_d: Some(318.5),
                branch_d: Some(89.1),
                e: Some(114.7),
                rib_b: Some(60.0),
                rib_h: Some(50.0),
                rib_n1: Some(2.0),
                rib_n2: Some(1.0),
                ..Default::default()
            };
            match f {
                0 => r.t = Some(v),
                1 => r.main_d = Some(v),
                2 => r.branch_d = Some(v),
                3 => r.e = Some(v),
                4 => r.rib_b = Some(v),
                _ => r.rib_n1 = Some(v),
            }
            let out = order::check_tee_rib(&r);
            assert!(
                out.pieces
                    .iter()
                    .all(|p| finite_all(&[p.area, p.kg, p.gross_kg])),
                "{r:?}"
            );
        }
    }
}
