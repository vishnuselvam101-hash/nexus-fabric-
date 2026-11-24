# Refusal codes

Engine 0.4.3 (21 Nov 2026): new code `branch_angle_not_verifiable` (work plan task 1.1). Engine 0.4.2 (review B2-005) reduced three refusals to their mathematical conditions, see the rows `reducer_near_cylindrical`, `developed_angle_overlaps`, `branch_angle_out_of_range` and `chord_error_unreachable`. Every refusal of a calculation or a correction request carries, in addition to its
message, the table's columns in the API answer:

- `code`: the stable snake_case rule, one per distinct rule (this table).
- `fields`: the request fields the rule concerns (the names the form sends). Empty when no single field is concerned.
- `message`: the text the API has always given. `error` carries the same text, so a client that reads only `error`
  keeps working.
- `detail`: the exact limit (`limit`) and the measured value (`measured`), present only in the rows marked "yes".
  A rule whose limit is sampled from the geometry, or stated only as a range in words, gives no detail: nothing is
  approximated.

The calculation answer of a refused job (HTTP 422) also carries `state` (REFUSED), `outcome` and `field` as before.
A refused correction (HTTP 422) carries the code of the corrected request's refusal. Authentication, permission and
not-found answers are not refusals of a request and carry no code, as before.

Implementation limits (magnitude, smallest dimension, chord error, angle, near-tangent and near-cylindrical limits) are
not engineering rules; the messages say so. Thickness, allowances, weld gap and kerf are not modelled, so no rule here
depends on them.

| code | rule | fields | detail |
|---|---|---|---|
| `request_not_object` | the request body is a JSON value that is not an object | none | no |
| `family_unknown` | the family is missing or is not one of the five families | `family` | no |
| `field_missing` | a field that the family requires is absent | the absent field | no |
| `field_wrong_type` | a field holds a value of the wrong JSON type, such as text where a number is required | the field(s) with the wrong type | no |
| `field_unknown` | a field is not one the family has; nothing is ignored silently (a `null` joint counts as absent) | the unknown field(s) | no |
| `surface_reference_required` | the shared `surface_reference` is absent and a part surface is absent too | `surface_reference`, `header_surface_reference`, `branch_surface_reference` | no |
| `request_malformed` | the body is not JSON, or it is not sent as `application/json` (API) | none | no |
| `value_not_finite` | a numeric value is not a finite number (NaN or infinity; in a correction, a `new_value` that is not a number) | the value's field | no |
| `value_not_positive` | a dimension that must be positive is zero or negative | the dimension's field | no |
| `offset_negative` | the eccentric offset `e` is negative | `e` | no |
| `magnitude_above_limit` | a value's magnitude exceeds 1e8 mm, the largest size the build is exercised with | the value's field | yes: `limit` 1e8, `measured` the magnitude |
| `dimension_below_limit` | a positive dimension is below 1e-6 mm, the smallest size the build is exercised with | the dimension's field | yes: `limit` 1e-6, `measured` the value |
| `reducer_radii_order` | a reducer needs `r1` greater than `r2` (equal radii are a cylinder; an oblique cylinder is not built) | `r1`, `r2` | no |
| `reducer_near_cylindrical` | the apex radius (r1 / sin α for the cone; the largest distance from the apex to the large end for the eccentric reducer) exceeds 1e8 mm, the largest size this build is exercised with. Until 0.4.1 the rule was sin α < 1e-3; the closed forms are exact for every sin α > 0, only the developed size grows | `r1`, `r2`, `h` | yes: `limit` 1e8 mm, `measured` the apex radius |
| `developed_angle_overlaps` | the seam gap at the small end, l2·(2π − Φ) for the cone (rho(0)·r2/r1·(2π − Ψ) for the eccentric reducer), is at or below 1e-9 mm, the closure resolution of the profile check (G11), so the two seam edges cannot be told apart. A cone sector never overlaps itself (its angle is below a full turn); until 0.4.1 the rule was 99.9 % of a turn | `r1`, `r2`, `h` (and `e` for the eccentric reducer) | yes: `limit` 1e-9 mm, `measured` the seam gap in mm |
| `branch_angle_out_of_range` | beta is not strictly between 0 and 180 degrees (not finite counts as not finite), or the developed branch (s_max, or s_max − s_min) exceeds 1e8 mm. Until 0.4.1 the rule was also sin β ≤ 0.17; the saddle s(φ) is exact for every 0 < β < 180 | `beta_deg` | for the size rule: `limit` 1e8 mm, `measured` the size; otherwise no |
| `branch_angle_not_verifiable` | (engine 0.4.3, work plan task 1.1) the independent verification cannot confirm the pattern at this branch angle: the bound on the verifier's own rounding error in a 3D-intersection check (it grows as 1/sin²(beta), near 0 and near 180 degrees) is not below the part of that check's tolerance left after the engine's own error, `agree + 3·bound`, AND at least one check of gate G10 fails, every failing check being one of the four 3D-intersection point checks with its OWN bound exceeded and a difference no larger than that bound plus the engine's bound (any other failure is a wrong pattern: G10 rejects the job). The verifier's criterion evaluated before the job is accepted, restricted to the regime where its rounding is not bounded inside its tolerance (derivation: `docs/backend/TOLERANCE_POLICY.md` section 7). In that regime a pattern the verifier confirms is accepted; outside it a failing check rejects the job (G10) as before. Not an engineering rule | `beta_deg` | yes: `limit` the tolerance budget agree + 3·bound (mm), `measured` the bound on the verifier's own rounding (mm), of the check where their ratio is largest |
| `branch_tangent_or_partial` | |delta| plus the branch radius reaches the header radius within 1e-6 of it: tangent and partial intersections, including the equal-diameter tee, are not implemented | `header_radius`, `branch_radius`, `delta` | yes: `limit` header radius x (1 - 1e-6), `measured` |delta| + branch radius |
| `square_end_too_short` | the square end `s_end` does not lie beyond the saddle | `s_end` | no (the saddle height is sampled) |
| `opening_outside_shell` | the branch opening does not lie inside the header length | `branch_station`, `header_length` | no (the span is sampled) |
| `chord_error_unreachable` | the curve cannot be represented within the 0.01 mm chord error with the most segments the build uses (46 080). For a full-turn polyline the sagitta R(1 − cos(π/n)) ≤ 0.01 mm bounds the developed radius to R ≤ 0.02·n²/π² ≈ 4.3e6 mm. Kept in 0.4.2 (review B2-005 point e) | the fields of that curve | no |
| `radius_dev_exceeds_outside` | the surface is `outside` and the development radius is larger than the contact radius (a negative wall) | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `radius_dev_below_inside` | the surface is `inside` and the development radius is smaller than the contact radius (a negative wall) | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `radius_neutral_mismatch` | the surface is `neutral` and the development radius differs from the contact radius | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `surface_reference_invalid` | a surface reference is not `outside`, `inside`, `neutral` or `unknown` (branch stub, header with branch, and since task 1.4 cylinder shell, cone and eccentric reducer) | the reference's field | no |
| `joint_pairing_invalid` | the joint is `set_on` but the branch surface is not `inside` or the header surface is not `outside` | `joint`, `header_surface_reference`, `branch_surface_reference` | no |
| `joint_type_unknown` | the joint is neither absent nor `set_on` | `joint` | no |
| `bend_angle_out_of_range` | the bend deduction is defined for 0 < theta <= 90 degrees; no request family uses it in this build | none | no |
| `reason_required` | a correction gives no reason, or one shorter than five characters once trimmed (work plan 3.2: counted in characters, that is Unicode scalar values after Rust's `str::trim`, never in bytes or UTF-16 units; a Japanese character, and a character outside the BMP, counts as one; the client counts the same way, `web/app.js` `charCount`/`trimRs`. Until the server hardening of `868402d` the length was the UTF-8 byte count, which let a reason of two Japanese characters pass) | `reason` | no |
| `reason_too_long` | a correction reason is longer than 2000 characters (counted as for `reason_required`) | `reason` | no |
| `correction_not_for_family` | corrections with propagation are implemented for the header-with-branch family only | `family` | no |
| `correction_unknown_parameter` | the parameter is not a numeric parameter of the job | `parameter` | no |
| `correction_value_unchanged` | the new value equals the current value | `new_value` | no |
| `correction_parameter_not_correctable` | the parameter cannot be changed by a correction | `parameter` | no |
| `correction_parameter_not_recorded` | the job has no record of the parameter, so the correction cannot be propagated | `parameter` | no |
| `correction_on_superseded_job` | the result is already superseded; the current one is corrected | none | no |
| `correction_on_refused_job` | the request was refused and has no result to correct | none | no |
| `correction_needs_recompute` | the result failed its integrity check or was computed by a superseded engine | none | no |
| `stored_result_unreadable` | the stored result cannot be read | none | no |
| `stored_request_unreadable` | the stored request cannot be read | none | no |
| `corrected_result_failed_verification` | the corrected request produced a pattern that failed its own verification | none | no |
| `no_pattern_produced` | a refused answer has no stated rule (a defensive code; the engine states a rule for every refusal) | none | no |

The `detail` values are those the engine computes: for example the magnitude limit is the constant `MAX_MAGNITUDE_MM`,
and the measured value is the value of the request. The table is checked against the codes of this build by
`crates/nf-core/tests/refusal_codes.rs` (`table_lists_every_code_of_the_build_once`).
