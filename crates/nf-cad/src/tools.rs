//! AMOEBA tool kinds for NF CAD. Each CAD operation is a tool in the operator registry, with typed arguments
//! (unknown fields refused) and a CAD target (an entity id or a canvas point), so the planner can drive it and
//! the cursor can point at it. All tools are read-only: none edits geometry, approves, or releases.

use crate::geom::Geom;
use crate::{Drawing, Measure, RuleCheck};
use serde::Deserialize;
use nf_amoeba_operator::events::{Event, Target};
use nf_amoeba_operator::operator::{Ctx, Registry, ToolOut};
use serde_json::{Value, json};

/// The open drawing, its quality checks and the selection, kept in the operator's extension slot for the run.
#[derive(Default)]
pub struct CadState {
    pub drawing: Option<Drawing>,
    pub checks: Vec<RuleCheck>,
    pub selection: Vec<String>,
}

impl CadState {
    /// Opens the bytes and runs the quality checks on them.
    pub fn load(bytes: &[u8]) -> Result<CadState, crate::Refusal> {
        Ok(CadState {
            drawing: Some(crate::open(bytes)?),
            checks: crate::quality_checks(bytes),
            selection: Vec::new(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectArgs {
    layer: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PickArgs {
    at: [f64; 2],
    tol: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MeasureArgs {
    Distance { from: [f64; 2], to: [f64; 2] },
    Vector { from: [f64; 2], to: [f64; 2] },
    LineAngle { a: [[f64; 2]; 2], b: [[f64; 2]; 2] },
    VertexAngle { at: [f64; 2], a: [f64; 2], b: [f64; 2] },
    ArcRadius { entity: String },
    Length { entity: String },
    Area { entity: String },
}

fn parse<T: for<'de> Deserialize<'de>>(a: &Value) -> Result<T, String> {
    serde_json::from_value(a.clone()).map_err(|e| format!("bad arguments: {e}"))
}

fn drawing<'a>(ctx: &'a Ctx<'_>) -> Result<&'a Drawing, String> {
    ctx.ext
        .get::<CadState>()
        .and_then(|s| s.drawing.as_ref())
        .ok_or_else(|| "no drawing is open".to_string())
}

fn out(events: Vec<Event>, result: Value) -> Result<ToolOut, String> {
    Ok(ToolOut { events, result })
}

fn ev(tool: &str, action: &str, target: Target, reason: &str, value: Value) -> Event {
    let mut e = Event::new(tool, action, target, reason).value(value);
    e.tool = tool.to_string();
    e
}

fn open_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let d = drawing(ctx)?;
    let result = json!({
        "version": d.version,
        "units": d.units,
        "layers": d.layers.len(),
        "entities": d.entities.len(),
        "extents": d.extents,
        "warnings": d.warnings,
    });
    out(vec![ev("cad_open", "look", Target::Cad { canvas: None, entity: None }, "drawing opened", result.clone())], result)
}

fn layers_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let d = drawing(ctx)?;
    let result = json!(d.layers);
    out(vec![ev("cad_layers", "look", Target::Cad { canvas: None, entity: None }, "layers listed", result.clone())], result)
}

fn select_tool(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let args: SelectArgs = parse(a)?;
    let ids: Vec<String> = drawing(ctx)?.on_layer(&args.layer).into_iter().map(str::to_string).collect();
    let n = ids.len();
    ctx.ext.get_or_insert_with(CadState::default).selection = ids.clone();
    let result = json!({ "layer": args.layer, "count": n, "entities": ids });
    out(vec![ev("cad_select", "click", Target::Cad { canvas: None, entity: None }, "layer selected", result.clone())], result)
}

fn pick_tool(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let args: PickArgs = parse(a)?;
    if !(args.tol.is_finite() && args.tol >= 0.0) {
        return Err("tol must be a finite number, zero or more".into());
    }
    let hit = drawing(ctx)?.pick(args.at, args.tol).map(|e| e.id.clone());
    let result = json!({ "entity": hit });
    let target = Target::Cad { canvas: Some(args.at), entity: hit.clone() };
    out(vec![ev("cad_pick", "point", target, "entity picked at canvas point", result.clone())], result)
}

fn measure_tool(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let args: MeasureArgs = parse(a)?;
    let (m, target) = match args {
        MeasureArgs::Distance { from, to } => (Measure::Distance { from, to }, Target::Cad { canvas: Some(from), entity: None }),
        MeasureArgs::Vector { from, to } => (Measure::Vector { from, to }, Target::Cad { canvas: Some(from), entity: None }),
        MeasureArgs::LineAngle { a, b } => (Measure::LineAngle { a, b }, Target::Cad { canvas: None, entity: None }),
        MeasureArgs::VertexAngle { at, a, b } => (Measure::VertexAngle { at, a, b }, Target::Cad { canvas: Some(at), entity: None }),
        MeasureArgs::ArcRadius { entity } => (Measure::ArcRadius { entity: entity.clone() }, Target::Cad { canvas: None, entity: Some(entity) }),
        MeasureArgs::Length { entity } => (Measure::Length { entity: entity.clone() }, Target::Cad { canvas: None, entity: Some(entity) }),
        MeasureArgs::Area { entity } => (Measure::Area { entity: entity.clone() }, Target::Cad { canvas: None, entity: Some(entity) }),
    };
    let values = drawing(ctx)?.measure(&m)?;
    let result = json!(values.iter().map(|v| json!({ "value": v.value, "unit": v.unit, "from": v.from })).collect::<Vec<_>>());
    out(vec![ev("cad_measure", "compute", target, "measured", result.clone())], result)
}

fn check_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let st = ctx.ext.get::<CadState>().ok_or("no drawing is open")?;
    let result = json!(st.checks);
    let pass = st.checks.iter().all(|c| c.pass);
    out(vec![ev("cad_check", "look", Target::Cad { canvas: None, entity: None }, "quality checks read", json!({ "pass": pass }))], result)
}

/// Registers every CAD tool kind under its name.
pub fn register(r: &mut Registry) {
    r.register("cad_blocks", Box::new(blocks_tool));
    r.register("cad_dimensions", Box::new(dimensions_tool));
    r.register("cad_hatches", Box::new(hatches_tool));
    r.register("cad_propose_edit", Box::new(propose_edit_tool));
    r.register("cad_open", Box::new(open_tool));
    r.register("cad_layers", Box::new(layers_tool));
    r.register("cad_select", Box::new(select_tool));
    r.register("cad_pick", Box::new(pick_tool));
    r.register("cad_measure", Box::new(measure_tool));
    r.register("cad_check", Box::new(check_tool));
}

/// INSERT and DIMENSION entities with how many entities each expanded beneath it. Read-only.
fn blocks_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let d = drawing(ctx)?;
    let rows: Vec<Value> = d
        .entities
        .iter()
        .filter(|e| e.kind == "INSERT" || e.kind == "DIMENSION")
        .map(|e| {
            let children = d.entities.iter().filter(|c| c.parent.as_deref() == Some(e.id.as_str())).count();
            json!({ "id": e.id, "kind": e.kind, "layer": e.layer, "children": children, "warnings": e.warnings })
        })
        .collect();
    let result = json!(rows);
    out(vec![ev("cad_blocks", "look", Target::Cad { canvas: None, entity: None }, "blocks listed", result.clone())], result)
}

/// DIMENSION text as shown (measured value substituted). Read-only.
fn dimensions_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let d = drawing(ctx)?;
    let rows: Vec<Value> = d
        .entities
        .iter()
        .filter(|e| e.kind == "DIMENSION")
        .filter_map(|e| match &e.geom {
            Geom::Text { at, text, .. } => Some(json!({ "id": e.id, "text": text, "at": at })),
            _ => None,
        })
        .collect();
    let result = json!(rows);
    out(vec![ev("cad_dimensions", "look", Target::Cad { canvas: None, entity: None }, "dimensions read", result.clone())], result)
}

/// HATCH boundaries with their enclosed area (boundary loops are read-only). Read-only.
fn hatches_tool(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let d = drawing(ctx)?;
    let rows: Vec<Value> = d
        .entities
        .iter()
        .filter(|e| e.kind == "HATCH")
        .map(|e| {
            let area = d.measure(&Measure::Area { entity: e.id.clone() }).ok().and_then(|v| v.first().map(|x| x.value));
            json!({ "id": e.id, "layer": e.layer, "area": area, "warnings": e.warnings })
        })
        .collect();
    let result = json!(rows);
    out(vec![ev("cad_hatches", "look", Target::Cad { canvas: None, entity: None }, "hatch boundaries read", result.clone())], result)
}

/// Turns the model's edit into a PROPOSAL for the person. It validates the operations' shape and the entity ids,
/// then returns them with an `ask` event. It never applies anything: the drawing, the DB and the versions are
/// untouched, and only a person's request through the edit route creates a version.
fn propose_edit_tool(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        ops: Vec<crate::edit::Op>,
        question: String,
    }
    let args: Args = parse(a)?;
    if args.question.trim().is_empty() || args.question.len() > 500 {
        return Err("question must be 1 to 500 characters".into());
    }
    if args.ops.is_empty() || args.ops.len() > 50 {
        return Err("propose between 1 and 50 operations".into());
    }
    let d = drawing(ctx)?;
    for id in crate::edit::touched_ids(&args.ops) {
        match d.entity(&id) {
            None => return Err(format!("no entity '{id}'")),
            Some(e) if e.parent.is_some() => return Err(format!("'{id}' is inside a block: propose on its INSERT")),
            Some(_) => {}
        }
    }
    let proposal = json!({ "ops": args.ops, "question": args.question, "applied": false });
    let e = ev("cad_propose_edit", "ask", Target::Cad { canvas: None, entity: None }, "edit proposed as a question", proposal.clone());
    let mut e = e;
    e.flags.push("proposal".into());
    out(vec![e], proposal)
}
