//! NEXUS FABRIC core: AMOEBA record model (first subset) and the development job.
//!
//! Implemented here:
//! - immutable, content-addressed records in the seven families of the
//!   consolidated record model (identity = SHA-256 of canonical JSON; no
//!   wall-clock data in identities);
//! - typed relations between records and the release closure with its hash;
//! - a deterministic development job (develop → verify → profile → DXF →
//!   re-import → gates) whose every step is a recorded Activity with logical
//!   work units;
//! - release-state evaluation. `APPROVED` is not reachable in this build:
//!   `release_state` cannot return it, whatever the gate table says, because
//!   engineer approval (G16) is not implemented;
//! - typed dependency edges, propagation of a change along them and scrutiny
//!   obligations for everything affected (`deps`, provisional rule table);
//! - an engine identifier derived from the engine sources at build time and
//!   recorded with every result.
//!
//! Not implemented (see docs/IMPLEMENTATION_STATUS.md): scheduler/rounds,
//! exploration, investigations, coverage ledger, lanes/overlays, quarantine,
//! engineer approval.

pub mod bend;
pub mod conebranch;
pub mod deps;
pub mod job;
pub mod order;
pub mod order_compute;
pub mod reading;
pub mod shapes;
pub mod tee;
pub mod tee_pad;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Identifier of the engine that computes results: a release number that is
/// raised by hand with every change of algorithm or output, plus a fingerprint
/// of the uem, nf-dxf and nf-core sources taken at build time (build.rs), so
/// that a result of one build can never be mistaken for a result of another.
/// It is recorded in every result and in every activity record.
///
/// 0.3.0 (19 Nov 2026): branch patterns are drawn from the OUTSIDE like header
/// plates (validation defect D1; up to 0.2.0 they were the mirror image), and
/// every pattern declares its marking side. Results of 0.2.0 and earlier are
/// results of another engine: REJECTED with gate G21, "recompute".
///
/// 0.4.0 (20 Nov 2026): openings are written as closed profiles (flag 70 = 1, no
/// repeated vertex); every DXF passes the DXF quality gate or is withheld with G15
/// FAIL (review B, RB-008 and item 5); header_with_branch takes per-part surface
/// references and a declared joint type (set-on, review B RB-001), and the
/// interface of an undeclared joint is no longer called set-on; the joint is also
/// checked by the independent 3D check. Results of 0.3.0 are REJECTED with G21.
///
/// 0.4.1 (20 Nov 2026): every DXF states its units by a key line (`NF-UNITS: mm`), and a
/// header-with-branch part states its joint ("set-on" or "not declared"), its header and branch surfaces
/// (`NF-JOINT`, `NF-HEADER-SURFACE`, `NF-BRANCH-SURFACE`, and two TEXT lines); the re-import check and the
/// quality gate read the statement back and compare it with the job (BH-004, BH-005). Every refusal carries a
/// stable code, the fields it concerns and, where the engine computes one exactly, the limit (BH-014). The
/// cut geometry is unchanged. Results of 0.4.0 are REJECTED with G21 "recompute".
///
/// 0.4.2 (20 Nov 2026): review B2 fixes B2-001..007: developed radius bounded by the wall (B2-001); polylines report their own deviation (B2-002); arc display chords by sagitta (B2-003); header gross/net area (B2-004); refusals reduced to their mathematical conditions — apex radius ≤ 1e8 mm instead of sin α ≥ 1e-3, seam gap > 1e-9 mm instead of 99.9 % of a turn, β strictly in (0,180) instead of sin β > 0.17; the chord cap stays and states its ~4.3e6 mm bound (B2-005); eccentric area by the adaptive rule and branch s_max/s_min by golden section (B2-006); eps_rep declared an estimate (B2-007); the stated numerical bound eps_num of a branch includes the rounding of the angle, which dominates at the shallow angles B2-005 now accepts. Quantities and refusal outcomes change; cut geometry of every previously accepted case is unchanged. Results of 0.4.1 are REJECTED with G21 recompute.
///
/// 0.4.3 (21 Nov 2026): work plan task 1.1. A branch angle the independent verification cannot confirm is refused
/// with its own code, `branch_angle_not_verifiable`, naming `beta_deg`, instead of a result that gate G10 then
/// rejects: the verifier's own criterion, evaluated on the final patterns before the job is accepted, in the regime
/// where the a-priori bound on the verifier's rounding (1/sin²(beta), near 0 and 180 degrees) is not below the part
/// of its tolerance left after the engine's own error, restricted to failures of the four 3D point checks that this
/// conditioning explains (docs/backend/TOLERANCE_POLICY.md section 7). A part whose
/// 8·eps_num reaches the tolerance of its 3D point check (near 180 degrees) carries `eps_num_governs` and the interface
/// shows eps_num prominently. Refusal outcomes change (jobs G10 rejected before are now refused); cut geometry of
/// every accepted case is unchanged. Results of 0.4.2 are REJECTED with G21 recompute.
/// 0.4.4 (21 Nov 2026): work plan task 1.2, a true error bound everywhere. `eps_rep` of every polyline is the
/// certified upper bound of its chord error (interval arithmetic on the closed-form derivatives, TOLERANCE_POLICY
/// section 8). `eps_rep_certified` says so, and only for the branch saddle and the header opening: the eccentric
/// reducer's vertex term contains a quadrature estimate, so its value is NOT certified and its two circumference checks
/// are INCONCLUSIVE (a would-be PASS only; a FAIL stays FAIL). Where no bound can be proved the checks using `eps_rep`
/// are INCONCLUSIVE in the same way. The developed-length checks of the branch saddle and the header opening use a
/// certified enclosure of the exact length instead of the Richardson limit. The area check of the header opening is
/// scaled to the opening's own extents (x and y) and computed relative to its first vertex; `opening_area` is computed
/// the same way. Cut geometry is byte-identical to 0.4.3; the quantities and check verdicts listed in the work plan
/// changed. Results of 0.4.3 are REJECTED with G21 recompute.
/// 0.4.5 (22 Nov 2026): work plan task 1.3, every convention stated in the DXF and the result. Every part states its
/// plate conventions (viewing side, offset sign and positive direction, rolling axis, seam and end edges, angular datum
/// and direction, plate origin and axes, kerf not compensated) as `NF-` 999 keys and as TEXT on NF_NOT_RELEASED; the
/// quality gate reads them back (rule `convention_statement`) and withholds a file that does not state exactly them.
/// RB-008: the outer profile of a part made of lines and polylines is one closed polyline (same vertices, same order;
/// a profile with an arc stays one entity per curve: an arc needs a bulge encoding this build does not write). The cut geometry
/// of every other entity is byte-identical. Results of 0.4.4 are REJECTED with G21 recompute.
/// 0.4.6 (work plan task 1.4, RB-010): the surface the entered radius refers to is declared for the cylinder shell, the
/// cone and the eccentric reducer as for the branch families, and an `unknown` surface makes the radius-dependent
/// checks of every family INCONCLUSIVE (never PASS; a FAIL stays FAIL), with a stated reason. Results with `unknown` on
/// a branch stub or a header with branch change verdicts (PASS -> INCONCLUSIVE). Requests without the field keep their
/// output, except that a branch stub without the field read `unknown` by its default and so changes in the same way.
/// Results of 0.4.5 are REJECTED with G21 recompute.
/// 0.4.7 (work plan task 1.5a, RC-09/RC-10, RB-006): the cylinder shell (`verify_cylinder` and `verify_cylinder_solid`)
/// and the eccentric reducer carry the `outline_counter_clockwise` guard as the branch and the header do: a mirrored
/// (clockwise) development FAILs it. Geometry is unchanged; the check list changed. Results of 0.4.6 are REJECTED with G21 recompute.
/// 0.4.8 (22 Nov 2026): work plan task 1.5, Review C fixes. The cylinder pipeline runs the solid verifier and two
/// plate-axis datum checks, so a plate turned 90 degrees FAILs (RC-09); the cone and eccentric `area` checks compare
/// the exact signed area of the DRAWN outline against the 3D surface, the apex-free congruence is directed (Kabsch,
/// a mirror or distance-preserving bulge is named), and both families gain seam datums, the cone also the CCW guard
/// (RC-10); the self-intersection checks flag touching and short overlaps (RC-13); the verifier re-derives the
/// engine-error bounds itself, bit-equal (RC-15). The reducer's area allowance is eps_rep-based and this family's
/// eps_rep is uncertified, so genuine reducers are INCONCLUSIVE by design (a FAIL stays FAIL); the cone's area
/// allowance gains the written-vertex term. The check list changed (compat_verdicts: 35 cylinder cases gain five
/// checks, 55 cones and 40 reducers gain two each, the reducer `area` flips PASS -> INCONCLUSIVE in all 40);
/// no other verdict or reason moved. Cut geometry is byte-identical to 0.4.7 (`develop.rs` unchanged).
/// Results of 0.4.7 are REJECTED with G21 recompute.
/// 0.4.9 (23 Nov 2026): text the engine shows is readable and in one language. A refused request's record carried
/// its outcome as the Rust debug form of the error (`OutOfRange { code: .. }`); it now carries the same sentence as
/// the message. The surface-unknown reason was written in English and Japanese in one string; the engine now writes
/// English only and the client translates it like every other engine sentence. Geometry and verdicts are unchanged;
/// record text changed, so results of 0.4.8 are REJECTED with G21 recompute.
/// 0.4.12 (25 Nov 2026): the display sagitta of the arc points is relative for small and large parts: min(0.01 mm, 1e-5 x M),
/// M the bounding-box diagonal of the pattern (a tiny part gets chords that follow its own size), and every arc shows at
/// least two chords, so the digest declares it as an arc. Cut geometry and the DXF are unchanged; the JSON arc points and
/// the verdicts that read them move. Results of 0.4.11 are REJECTED with G21 recompute.
/// 0.4.11 (24 Nov 2026): AMOEBA digest. Every DXF of a job is re-measured by nf-digest, an independent reader, and
/// the digest is kept as an immutable snapshot; gate G23 (AMOEBA digest) reads it and a DXF is served only when every
/// check passes. The engine's geometry, checks and verdicts are unchanged. Results of 0.4.10 are REJECTED with G21 recompute.
/// 0.4.10 (23 Nov 2026): wording only. The G14 detail says "Algorithms are evidence level EV-1 (not independently
/// reviewed) and not production-qualified." (the authorship wording was removed from every text of the product);
/// geometry, checks and verdicts unchanged; record text changed, so results of 0.4.9 are REJECTED with G21 recompute.
/// 0.5.0 (28 Nov 2026): NFJ-1 wiring lane L10. The shape maths of the zumen-kirizu port (`shapes`, `bend`, `tee`,
/// `tee_pad`, `conebranch`) is wired to orders by `order_compute`: every result carries `certified`, `method`, the
/// named rules, holds, questions, `eps_rep` and the thickness convention; every hold blocks the cut file. The five
/// families, their cut geometry, checks and verdicts are unchanged (compat corpus and verdict snapshot byte-identical);
/// the engine identifier changes because the engine sources do, and the new refusal codes (`shape_*`, `bend_*`, `tee_*`,
/// `pad_*`, `conebranch_*`, `ring_*`, `plate_*`, `rib_*`, `cone_*`, `ecc_*`) join docs/backend/REFUSAL_CODES.md.
/// Results of 0.4.12 are REJECTED with G21 recompute.
pub const ENGINE_VERSION: &str = concat!("nf-engine 0.5.0+src.", env!("NF_ENGINE_SRC_HASH"));

/// Engine identifiers known to have produced wrong or unidentifiable results.
/// Stored results carrying one of these (or any identifier other than the
/// running engine's) are not trusted: they must be recomputed.
pub const SUPERSEDED_ENGINES: [(&str, &str); 1] = [(
    "nf-core 0.1.0 / uem 0.1.0",
    "This identifier was used unchanged by every build up to 2026-11-19, including builds before the header-outline fix (docs/LEARNING_AND_STRESS_TESTING.md, Defect 002: header plate outline and branch opening in different frames). A result carrying it cannot be told apart from one computed with that defect.",
)];

pub fn superseded_engine_reason(engine: &str) -> Option<&'static str> {
    SUPERSEDED_ENGINES
        .iter()
        .find(|(id, _)| *id == engine)
        .map(|(_, why)| *why)
}

/// Only results computed by exactly the running engine are current.
pub fn engine_is_current(engine: &str) -> bool {
    engine == ENGINE_VERSION && superseded_engine_reason(engine).is_none()
}

/// Machine-readable refusals: one stable code per rule (docs/backend/REFUSAL_CODES.md, which a test checks
/// against these constants). The engine's codes are re-exported from `uem::refusal`.
pub mod refusal {
    pub use uem::refusal::*;

    // the request structure, as the API reads it (400)
    pub const REQUEST_NOT_OBJECT: &str = "request_not_object";
    pub const FAMILY_UNKNOWN: &str = "family_unknown";
    pub const FIELD_MISSING: &str = "field_missing";
    pub const FIELD_WRONG_TYPE: &str = "field_wrong_type";
    pub const FIELD_UNKNOWN: &str = "field_unknown";
    pub const SURFACE_REFERENCE_REQUIRED: &str = "surface_reference_required";
    pub const REQUEST_MALFORMED: &str = "request_malformed";

    // a correction request (400, 409, 422)
    pub const REASON_REQUIRED: &str = "reason_required";
    pub const REASON_TOO_LONG: &str = "reason_too_long";
    pub const CORRECTION_NOT_FOR_FAMILY: &str = "correction_not_for_family";
    pub const CORRECTION_UNKNOWN_PARAMETER: &str = "correction_unknown_parameter";
    pub const CORRECTION_VALUE_UNCHANGED: &str = "correction_value_unchanged";
    pub const CORRECTION_PARAMETER_NOT_CORRECTABLE: &str = "correction_parameter_not_correctable";
    pub const CORRECTION_PARAMETER_NOT_RECORDED: &str = "correction_parameter_not_recorded";
    pub const CORRECTION_ON_SUPERSEDED_JOB: &str = "correction_on_superseded_job";
    pub const CORRECTION_ON_REFUSED_JOB: &str = "correction_on_refused_job";
    pub const CORRECTION_NEEDS_RECOMPUTE: &str = "correction_needs_recompute";
    pub const STORED_RESULT_UNREADABLE: &str = "stored_result_unreadable";
    pub const STORED_REQUEST_UNREADABLE: &str = "stored_request_unreadable";
    pub const CORRECTED_RESULT_FAILED_VERIFICATION: &str = "corrected_result_failed_verification";

    // a request that produced no pattern and no stated rule (defensive; not reached by the engine)
    pub const NO_PATTERN_PRODUCED: &str = "no_pattern_produced";

    /// Every code of this build, in the order of the table in docs/backend/REFUSAL_CODES.md.
    /// The refusal codes of the zumen-kirizu shape modules (`shapes`, `bend`, `tee`, `tee_pad`, `conebranch`), wired in 0.5.0.
    pub const SHAPE_CODES: [&str; 80] = [
        "shape_value_not_finite",
        "shape_value_not_positive",
        "shape_value_too_large",
        "ring_id_not_below_od",
        "shape_split_out_of_range",
        "cone_not_conical",
        "shape_wall_too_thick",
        "plate_taper_offset_missing",
        "plate_chamfer_too_long",
        "plate_chamfer_corner_invalid",
        "plate_hole_invalid",
        "rib_geometry_invalid",
        "ecc_quadrature_not_converged",
        "plate_hole_outside_outline",
        "plate_hole_overlap",
        "bend_value_not_finite",
        "bend_value_not_positive",
        "bend_value_too_large",
        "bend_wall_too_thick",
        "bend_angle_invalid",
        "bend_joints_invalid",
        "bend_joints_above_table",
        "bend_split_out_of_range",
        "bend_reducing_needs_two_joints",
        "bend_cone_too_steep",
        "bend_cut_degenerate",
        "bend_cuts_cross_at_heel",
        "bend_area_not_converged",
        "bend_result_not_finite",
        "bend_taper_piece_out_of_range",
        "bend_taper_piece_not_reducing",
        "tee_value_not_finite",
        "tee_value_not_positive",
        "tee_value_too_large",
        "tee_wall_too_thick",
        "tee_no_branch",
        "tee_branches_above_limit",
        "tee_seams_out_of_range",
        "tee_seam_angle_invalid",
        "tee_branch_angle_invalid",
        "tee_branch_too_large_to_insert",
        "tee_branch_bore_beyond_main",
        "tee_branch_end_inside_main",
        "tee_hole_off_pipe",
        "tee_hole_beyond_pipe_end",
        "tee_holes_overlap",
        "tee_offset_side_not_perpendicular",
        "tee_offset_negative_without_side",
        "tee_rib_on_lateral_unsupported",
        "tee_area_not_converged",
        "tee_result_not_finite",
        "pad_value_not_finite",
        "pad_value_not_positive",
        "pad_value_too_large",
        "pad_branch_beyond_main",
        "pad_hole_off_main",
        "pad_outline_not_around_hole",
        "pad_split_out_of_range",
        "pad_split_not_possible",
        "pad_result_not_finite",
        "conebranch_value_not_finite",
        "conebranch_value_not_positive",
        "conebranch_value_too_large",
        "conebranch_not_conical",
        "conebranch_wall_too_thick",
        "conebranch_branch_not_perpendicular",
        "conebranch_split_out_of_range",
        "conebranch_branch_not_within_cone",
        "conebranch_hole_not_through_plate",
        "conebranch_hole_beyond_mouth",
        "conebranch_branch_end_below_cut",
        "conebranch_cut_not_found",
        "conebranch_fan_not_converged",
        "conebranch_area_not_converged",
        "conebranch_seam_not_placeable",
        "conebranch_result_not_finite",
        "conebranch_seam_gap_negative",
        "conebranch_flat_not_applicable",
        "conebranch_cut_out_of_range",
        "conebranch_cut_not_continuous",
    ];

    pub const ALL: [&str; 122] = [
        REQUEST_NOT_OBJECT,
        FAMILY_UNKNOWN,
        FIELD_MISSING,
        FIELD_WRONG_TYPE,
        FIELD_UNKNOWN,
        SURFACE_REFERENCE_REQUIRED,
        REQUEST_MALFORMED,
        VALUE_NOT_FINITE,
        VALUE_NOT_POSITIVE,
        OFFSET_NEGATIVE,
        MAGNITUDE_ABOVE_LIMIT,
        DIMENSION_BELOW_LIMIT,
        REDUCER_RADII_ORDER,
        REDUCER_NEAR_CYLINDRICAL,
        DEVELOPED_ANGLE_OVERLAPS,
        BRANCH_ANGLE_OUT_OF_RANGE,
        BRANCH_ANGLE_NOT_VERIFIABLE,
        BRANCH_TANGENT_OR_PARTIAL,
        SQUARE_END_TOO_SHORT,
        OPENING_OUTSIDE_SHELL,
        CHORD_ERROR_UNREACHABLE,
        RADIUS_DEV_EXCEEDS_OUTSIDE,
        RADIUS_DEV_BELOW_INSIDE,
        RADIUS_NEUTRAL_MISMATCH,
        SURFACE_REFERENCE_INVALID,
        JOINT_PAIRING_INVALID,
        JOINT_TYPE_UNKNOWN,
        BEND_ANGLE_OUT_OF_RANGE,
        REASON_REQUIRED,
        REASON_TOO_LONG,
        CORRECTION_NOT_FOR_FAMILY,
        CORRECTION_UNKNOWN_PARAMETER,
        CORRECTION_VALUE_UNCHANGED,
        CORRECTION_PARAMETER_NOT_CORRECTABLE,
        CORRECTION_PARAMETER_NOT_RECORDED,
        CORRECTION_ON_SUPERSEDED_JOB,
        CORRECTION_ON_REFUSED_JOB,
        CORRECTION_NEEDS_RECOMPUTE,
        STORED_RESULT_UNREADABLE,
        STORED_REQUEST_UNREADABLE,
        CORRECTED_RESULT_FAILED_VERIFICATION,
        NO_PATTERN_PRODUCED,
        "shape_value_not_finite",
        "shape_value_not_positive",
        "shape_value_too_large",
        "ring_id_not_below_od",
        "shape_split_out_of_range",
        "cone_not_conical",
        "shape_wall_too_thick",
        "plate_taper_offset_missing",
        "plate_chamfer_too_long",
        "plate_chamfer_corner_invalid",
        "plate_hole_invalid",
        "rib_geometry_invalid",
        "ecc_quadrature_not_converged",
        "plate_hole_outside_outline",
        "plate_hole_overlap",
        "bend_value_not_finite",
        "bend_value_not_positive",
        "bend_value_too_large",
        "bend_wall_too_thick",
        "bend_angle_invalid",
        "bend_joints_invalid",
        "bend_joints_above_table",
        "bend_split_out_of_range",
        "bend_reducing_needs_two_joints",
        "bend_cone_too_steep",
        "bend_cut_degenerate",
        "bend_cuts_cross_at_heel",
        "bend_area_not_converged",
        "bend_result_not_finite",
        "bend_taper_piece_out_of_range",
        "bend_taper_piece_not_reducing",
        "tee_value_not_finite",
        "tee_value_not_positive",
        "tee_value_too_large",
        "tee_wall_too_thick",
        "tee_no_branch",
        "tee_branches_above_limit",
        "tee_seams_out_of_range",
        "tee_seam_angle_invalid",
        "tee_branch_angle_invalid",
        "tee_branch_too_large_to_insert",
        "tee_branch_bore_beyond_main",
        "tee_branch_end_inside_main",
        "tee_hole_off_pipe",
        "tee_hole_beyond_pipe_end",
        "tee_holes_overlap",
        "tee_offset_side_not_perpendicular",
        "tee_offset_negative_without_side",
        "tee_rib_on_lateral_unsupported",
        "tee_area_not_converged",
        "tee_result_not_finite",
        "pad_value_not_finite",
        "pad_value_not_positive",
        "pad_value_too_large",
        "pad_branch_beyond_main",
        "pad_hole_off_main",
        "pad_outline_not_around_hole",
        "pad_split_out_of_range",
        "pad_split_not_possible",
        "pad_result_not_finite",
        "conebranch_value_not_finite",
        "conebranch_value_not_positive",
        "conebranch_value_too_large",
        "conebranch_not_conical",
        "conebranch_wall_too_thick",
        "conebranch_branch_not_perpendicular",
        "conebranch_split_out_of_range",
        "conebranch_branch_not_within_cone",
        "conebranch_hole_not_through_plate",
        "conebranch_hole_beyond_mouth",
        "conebranch_branch_end_below_cut",
        "conebranch_cut_not_found",
        "conebranch_fan_not_converged",
        "conebranch_area_not_converged",
        "conebranch_seam_not_placeable",
        "conebranch_result_not_finite",
        "conebranch_seam_gap_negative",
        "conebranch_flat_not_applicable",
        "conebranch_cut_out_of_range",
        "conebranch_cut_not_continuous",
    ];
}

/// A refusal of a calculation or a correction request, in machine-readable form. `message` is the text the
/// engine or the API has always given; `fields` names the request fields the rule concerns; `detail` holds the
/// limit and the measured value where the engine computes them exactly (absent otherwise).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: String,
    pub fields: Vec<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<BTreeMap<String, f64>>,
}

impl Refusal {
    pub fn new(code: &str, fields: &[&str], message: impl Into<String>) -> Refusal {
        Refusal {
            code: code.to_string(),
            fields: fields.iter().map(|f| f.to_string()).collect(),
            message: message.into(),
            detail: None,
        }
    }
    /// The refusal of an engine error: its code, fields and detail, and the message as the error displays it.
    pub fn from_engine(e: &uem::UemError) -> Refusal {
        Refusal {
            code: e.code().to_string(),
            fields: e.fields().iter().map(|f| f.to_string()).collect(),
            message: e.to_string(),
            detail: detail_map(e.detail()),
        }
    }
}

/// The detail of a refusal as a JSON object; absent when the rule gives none.
pub fn detail_map(detail: &[(&str, f64)]) -> Option<BTreeMap<String, f64>> {
    if detail.is_empty() {
        None
    } else {
        Some(detail.iter().map(|(k, v)| (k.to_string(), *v)).collect())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Source,
    Claim,
    Relation,
    Activity,
    Finding,
    Assessment,
    Attestation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rel {
    /// Typed edge: this record depends on / refers to `to`.
    pub rel: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub family: Family,
    pub kind: String,
    pub payload: Value,
    pub relations: Vec<Rel>,
    /// Producing activity (record id), if any.
    pub activity: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    hex::encode(&d[..])
}

/// Canonical serialisation: serde_json with sorted object keys (the default
/// map type). Floats use the shortest round-trip representation, which is
/// deterministic for identical binary64 values.
fn canonical(v: &Value) -> String {
    serde_json::to_string(v).expect("json")
}

impl Record {
    pub fn new(
        family: Family,
        kind: &str,
        payload: Value,
        relations: Vec<Rel>,
        activity: Option<String>,
    ) -> Record {
        let body = json!({ "family": family, "kind": kind, "payload": payload, "relations": relations, "activity": activity });
        let id = sha256_hex(canonical(&body).as_bytes());
        Record {
            id,
            family,
            kind: kind.to_string(),
            payload,
            relations,
            activity,
        }
    }

    /// Recompute the identity from content (used by audits and on load).
    pub fn verify_id(&self) -> bool {
        Record::new(
            self.family,
            &self.kind,
            self.payload.clone(),
            self.relations.clone(),
            self.activity.clone(),
        )
        .id == self.id
    }
}

pub fn rel(name: &str, to: &str) -> Rel {
    Rel {
        rel: name.to_string(),
        to: to.to_string(),
    }
}

/// Smallest set containing `roots` and closed under relations (and producing
/// activities). Records missing from `records` are reported, never ignored.
pub fn closure(
    records: &BTreeMap<String, Record>,
    roots: &[String],
) -> (BTreeSet<String>, Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();
    let mut stack: Vec<String> = roots.to_vec();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        match records.get(&id) {
            None => missing.push(id),
            Some(r) => {
                for e in &r.relations {
                    stack.push(e.to.clone());
                }
                if let Some(a) = &r.activity {
                    stack.push(a.clone());
                }
            }
        }
    }
    (seen, missing)
}

/// Hash of a release closure: SHA-256 over the sorted record ids.
pub fn closure_hash(ids: &BTreeSet<String>) -> String {
    let joined: String = ids
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    sha256_hex(joined.as_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateResult {
    Pass,
    Fail,
    Inconclusive,
    NotImplemented,
    NotRun,
    NotApplicable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub id: String,
    pub name: String,
    pub critical: bool,
    pub result: GateResult,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseState {
    /// Defined by Spec 02 §10.3 and kept so that the name stays reserved.
    /// NOT REACHABLE in this build: no function produces it.
    Approved,
    ReviewRequired,
    Rejected,
    /// The engine refused the request and computed nothing. Not a result:
    /// there is nothing to review, correct, replay or download.
    Refused,
}

/// Gates every computed result must carry. A stored gate table that lacks one
/// of them is incomplete, and an incomplete table is a failed one.
pub const REQUIRED_GATES: [&str; 12] = [
    "G01", "G02", "G05", "G06", "G08", "G10", "G11", "G14", "G15", "G16", "G18", "G19",
];

/// Release state from gate results (Spec 02 §10.3). A model confidence score
/// is not an input to this function, and cannot be.
///
/// Fail closed: an empty gate table is REJECTED (no evidence is not good
/// evidence), and so is any critical FAIL. Spec 02 §10.3 would give APPROVED
/// when every gate passes; engineer approval is not implemented in this
/// build, so that case is REVIEW_REQUIRED here and APPROVED cannot be returned.
pub fn release_state(gates: &[Gate]) -> ReleaseState {
    if gates.is_empty()
        || gates
            .iter()
            .any(|g| g.critical && g.result == GateResult::Fail)
    {
        ReleaseState::Rejected
    } else {
        ReleaseState::ReviewRequired
    }
}

/// Complete a gate table against `REQUIRED_GATES`: every required gate that is
/// absent is added as a critical FAIL. Used wherever a table comes from
/// storage rather than from the computation that produced it.
pub fn with_required_gates(mut gates: Vec<Gate>) -> Vec<Gate> {
    for id in REQUIRED_GATES {
        if !gates.iter().any(|g| g.id == id) {
            gates.push(Gate { id: id.into(), name: "missing_gate".into(), critical: true, result: GateResult::Fail, detail: format!("Gate {id} is missing from the stored result. An incomplete gate table is treated as failed (integrity).") });
        }
    }
    gates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_identity_is_content_addressed() {
        let a = Record::new(Family::Claim, "x", json!({"b": 1, "a": 2.5}), vec![], None);
        let b = Record::new(Family::Claim, "x", json!({"a": 2.5, "b": 1}), vec![], None);
        assert_eq!(a.id, b.id, "key order must not matter");
        let c = Record::new(Family::Claim, "x", json!({"a": 2.5, "b": 2}), vec![], None);
        assert_ne!(a.id, c.id);
        assert!(a.verify_id());
        let mut tampered = a.clone();
        tampered.payload = json!({"a": 2.5, "b": 99});
        assert!(!tampered.verify_id());
    }

    #[test]
    fn closure_follows_relations_and_reports_missing() {
        let a = Record::new(Family::Claim, "a", json!(1), vec![], None);
        let b = Record::new(
            Family::Claim,
            "b",
            json!(2),
            vec![rel("computed_from", &a.id)],
            None,
        );
        let c = Record::new(
            Family::Claim,
            "c",
            json!(3),
            vec![rel("computed_from", &b.id), rel("x", "deadbeef")],
            None,
        );
        let unrelated = Record::new(Family::Claim, "u", json!(4), vec![], None);
        let mut m = BTreeMap::new();
        for r in [&a, &b, &c, &unrelated] {
            m.insert(r.id.clone(), r.clone());
        }
        let (ids, missing) = closure(&m, std::slice::from_ref(&c.id));
        assert!(ids.contains(&a.id) && ids.contains(&b.id) && !ids.contains(&unrelated.id));
        assert_eq!(missing, vec!["deadbeef".to_string()]);
        // unrelated additions do not change the closure hash
        let h1 = closure_hash(&ids);
        let more = Record::new(Family::Claim, "v", json!(5), vec![], None);
        m.insert(more.id.clone(), more);
        assert_eq!(
            closure_hash(&closure(&m, std::slice::from_ref(&c.id)).0),
            h1
        );
    }

    #[test]
    fn release_state_rules() {
        let g = |critical, result| Gate {
            id: "G".into(),
            name: "n".into(),
            critical,
            result,
            detail: String::new(),
        };
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(true, GateResult::Fail)]),
            ReleaseState::Rejected
        );
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(false, GateResult::Fail)]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[
                g(true, GateResult::Pass),
                g(true, GateResult::NotImplemented)
            ]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[g(true, GateResult::Pass), g(true, GateResult::Inconclusive)]),
            ReleaseState::ReviewRequired
        );
        // BE-002: this assertion used to expect APPROVED. APPROVED must be unreachable in this build
        // (G16, engineer approval, is not implemented), and "no gates" must fail closed.
        assert_eq!(
            release_state(&[g(true, GateResult::Pass)]),
            ReleaseState::ReviewRequired
        );
        assert_eq!(
            release_state(&[]),
            ReleaseState::Rejected,
            "an empty gate table is no evidence"
        );
    }

    /// BE-002: no gate table, however favourable or malformed, yields APPROVED.
    #[test]
    fn approved_is_unreachable_for_every_gate_table() {
        use GateResult::*;
        let all = [
            Pass,
            Fail,
            Inconclusive,
            NotImplemented,
            NotRun,
            NotApplicable,
        ];
        let g = |id: &str, critical, result| Gate {
            id: id.into(),
            name: "n".into(),
            critical,
            result,
            detail: String::new(),
        };
        let mut tables: Vec<Vec<Gate>> = vec![
            vec![],
            REQUIRED_GATES.iter().map(|id| g(id, true, Pass)).collect(),
            REQUIRED_GATES
                .iter()
                .map(|id| g(id, false, NotApplicable))
                .collect(),
        ];
        for a in all {
            for b in all {
                for critical in [true, false] {
                    tables.push(vec![g("G10", critical, a), g("G16", critical, b)]);
                }
            }
        }
        for t in &tables {
            assert_ne!(release_state(t), ReleaseState::Approved, "{t:?}");
            assert_ne!(
                release_state(&with_required_gates(t.clone())),
                ReleaseState::Approved
            );
        }
        // a table that lacks required gates is rejected once completed
        assert_eq!(
            release_state(&with_required_gates(vec![g("G20", true, Pass)])),
            ReleaseState::Rejected
        );
        assert_eq!(
            release_state(&with_required_gates(vec![])),
            ReleaseState::Rejected
        );
        let full: Vec<Gate> = REQUIRED_GATES.iter().map(|id| g(id, true, Pass)).collect();
        assert_eq!(
            with_required_gates(full.clone()),
            full,
            "a complete table is left as it is"
        );
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;

    /// Records are stored as JSON text. Identity must survive storage exactly,
    /// which requires exact float round-tripping (serde_json `float_roundtrip`).
    #[test]
    fn records_survive_json_storage_bit_exactly() {
        let asm = job::Request::HeaderWithBranch {
            header_radius: 150.0,
            header_dev_radius: 150.0,
            header_length: 1200.0,
            branch_station: 600.0,
            branch_radius: 100.0,
            branch_dev_radius: 100.0,
            beta_deg: 60.0,
            delta: 12.5,
            s_end: 400.0,
            surface_reference: Some("outside".into()),
            header_surface_reference: None,
            branch_surface_reference: None,
            joint: None,
        };
        let mut all = job::run(
            &job::Request::EccentricReducer {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
                e: 100.0,
                surface_reference: None,
            },
            "t",
        )
        .records;
        all.extend(job::run(&asm, "t").records);
        for r in &all {
            let text = serde_json::to_string(r).unwrap();
            let back: Record = serde_json::from_str(&text).unwrap();
            assert!(
                back.verify_id(),
                "record {} ({}) changed identity after a JSON round trip",
                r.kind,
                r.id
            );
        }
    }
}
