# Refusal codes

Engine 0.4.1 (20 Nov 2026). Every refusal of a calculation or a correction request carries, in addition to its
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
| `reducer_near_cylindrical` | the cone's sine of half-angle is below 1e-3, where the apex-free method is not implemented | `r1`, `r2`, `h` | yes: `limit` 0.001, `measured` the sine |
| `developed_angle_overlaps` | the developed angle reaches 0.999 of a full turn, so the sector would overlap itself | `r1`, `r2`, `h` (and `e` for the eccentric reducer) | yes: `limit` 0.999 x 2π rad, `measured` the developed angle in rad |
| `branch_angle_out_of_range` | the sine of the angle between the axes is not above 0.17 (about 10 to 170 degrees), or beta is not strictly between 0 and 180 degrees | `beta_deg` | no (the limit is stated on the sine and in words) |
| `branch_tangent_or_partial` | |delta| plus the branch radius reaches the header radius within 1e-6 of it: tangent and partial intersections, including the equal-diameter tee, are not implemented | `header_radius`, `branch_radius`, `delta` | yes: `limit` header radius x (1 - 1e-6), `measured` |delta| + branch radius |
| `square_end_too_short` | the square end `s_end` does not lie beyond the saddle | `s_end` | no (the saddle height is sampled) |
| `opening_outside_shell` | the branch opening does not lie inside the header length | `branch_station`, `header_length` | no (the span is sampled) |
| `chord_error_unreachable` | the curve cannot be represented within the 0.01 mm chord error with the most segments the build uses | the fields of that curve | no |
| `radius_dev_exceeds_outside` | the surface is `outside` and the development radius is larger than the contact radius (a negative wall) | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `radius_dev_below_inside` | the surface is `inside` and the development radius is smaller than the contact radius (a negative wall) | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `radius_neutral_mismatch` | the surface is `neutral` and the development radius differs from the contact radius | the development radius, the contact radius | yes: `limit` the contact radius, `measured` the development radius |
| `surface_reference_invalid` | a surface reference is not `outside`, `inside`, `neutral` or `unknown` | the reference's field | no |
| `joint_pairing_invalid` | the joint is `set_on` but the branch surface is not `inside` or the header surface is not `outside` | `joint`, `header_surface_reference`, `branch_surface_reference` | no |
| `joint_type_unknown` | the joint is neither absent nor `set_on` | `joint` | no |
| `bend_angle_out_of_range` | the bend deduction is defined for 0 < theta <= 90 degrees; no request family uses it in this build | none | no |
| `reason_required` | a correction gives no reason, or one shorter than five bytes (the UTF-8 length: a Japanese character counts as three bytes) | `reason` | no |
| `reason_too_long` | a correction reason is longer than 2000 characters | `reason` | no |
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
