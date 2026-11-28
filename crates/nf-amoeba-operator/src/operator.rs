//! The operator loop: brain proposes an action, the registry runs the tool, the tool's events go to the server.

use crate::events::{Event, Target};
use crate::eyes::PageRead;
use crate::layout;
use nf_amoeba_agent::agent::{Agent, Answer};
use nf_amoeba_agent::http::Transport;
use nf_core::assign::{self, Assignment, FamilyVerifier, Status};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- hands

/// The tool bridge as the operator sees it. `Agent` is the real one; a test can stand in.
pub trait Hands {
    fn call(&mut self, tool: &str, args: &Value) -> Result<Answer, String>;
    fn take_binary(&mut self) -> Option<Vec<u8>>;
}

impl<T: Transport> Hands for Agent<T> {
    fn call(&mut self, tool: &str, args: &Value) -> Result<Answer, String> {
        Agent::call(self, tool, args)
    }
    fn take_binary(&mut self) -> Option<Vec<u8>> {
        Agent::take_binary(self)
    }
}

/// A tool call that must succeed: a refusal of the server is an error carrying its code and message.
fn call_ok(h: &mut dyn Hands, tool: &str, args: Value) -> Result<Value, String> {
    let a = h.call(tool, &args)?;
    let Some(r) = a.result.filter(|_| a.ok) else {
        return Err(format!("{tool}: {}", a.error.unwrap_or_default()));
    };
    if let Some(rej) = r.get("rejected") {
        let code = rej["code"].as_str().unwrap_or("refused");
        let msg = rej["error"].as_str().unwrap_or_default();
        return Err(format!("{tool} refused ({code}): {msg}"));
    }
    Ok(r)
}

// ---------------------------------------------------------------- the job sheet

/// One unit of work: one page, one engine request (a family), the parts of that request in list-row order.
#[derive(Debug, Clone)]
pub struct UnitSpec {
    pub id: String,
    /// part number in the order
    pub no: String,
    pub name: String,
    pub family: String,
    pub page: i64,
    /// pattern part names of the family, in the order of the rows of the material list
    pub parts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub order: i64,
    pub project: i64,
    pub units: Vec<UnitSpec>,
    /// also run the order compute (for parts of the shape catalogue)
    pub order_compute: bool,
}

impl Manifest {
    pub fn from_json(v: &Value) -> Result<Manifest, String> {
        let int = |x: &Value, k: &str| x[k].as_i64().ok_or(format!("manifest: '{k}' is missing"));
        let units = v["units"]
            .as_array()
            .ok_or("manifest: 'units' is missing")?
            .iter()
            .map(|u| {
                Ok(UnitSpec {
                    id: u["id"].as_str().ok_or("manifest: unit 'id'")?.to_string(),
                    no: u["no"].as_str().ok_or("manifest: unit 'no'")?.to_string(),
                    name: u["name"].as_str().unwrap_or_default().to_string(),
                    family: u["family"]
                        .as_str()
                        .ok_or("manifest: unit 'family'")?
                        .to_string(),
                    page: int(u, "page")?,
                    parts: u["parts"]
                        .as_array()
                        .ok_or("manifest: unit 'parts'")?
                        .iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Manifest {
            order: int(v, "order")?,
            project: int(v, "project")?,
            units,
            order_compute: v["order_compute"].as_bool().unwrap_or(false),
        })
    }
}

// ---------------------------------------------------------------- tools, actions, brain

#[derive(Debug, Clone)]
pub struct Action {
    pub tool: String,
    pub args: Value,
}

impl Action {
    pub fn new(tool: &str, args: Value) -> Action {
        Action {
            tool: tool.into(),
            args,
        }
    }
}

pub struct ToolOut {
    pub events: Vec<Event>,
    pub result: Value,
}

/// What the brain sees of the last action.
pub struct Seen<'a> {
    pub tool: &'a str,
    pub result: &'a Value,
    pub failed: bool,
}

pub trait Brain {
    fn name(&self) -> String;
    fn next(&mut self, last: Option<Seen<'_>>) -> Option<Action>;
}

pub struct UnitState {
    pub spec: UnitSpec,
    pub assignment: Option<Assignment>,
    pub job: Option<i64>,
    pub job_state: Option<String>,
    pub held: Vec<String>,
    pub dxf_served: Vec<String>,
    pub written: bool,
}

pub struct Ctx<'a> {
    pub hands: &'a mut dyn Hands,
    pub manifest: &'a Manifest,
    pub pages: BTreeMap<i64, PageRead>,
    pub units: BTreeMap<String, UnitState>,
    /// Extension slot: a tool that needs state of its own (a CAD tool's canvas, entity table or view) keeps it here,
    /// keyed by its type, so a new tool family plugs in without any change to the loop or to `Ctx`.
    pub ext: Extensions,
    counter: u64,
}

/// Typed per-run state for tools that are not part of the drawing-reading core. One value per type.
#[derive(Default)]
pub struct Extensions(BTreeMap<std::any::TypeId, Box<dyn std::any::Any>>);

impl Extensions {
    /// Store `v`, replacing and returning an earlier value of the same type.
    pub fn insert<T: 'static>(&mut self, v: T) -> Option<T> {
        self.0
            .insert(std::any::TypeId::of::<T>(), Box::new(v))
            .and_then(|b| b.downcast().ok().map(|b| *b))
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0.get(&std::any::TypeId::of::<T>())?.downcast_ref()
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.get_mut(&std::any::TypeId::of::<T>())?.downcast_mut()
    }
    /// The value of type `T`, created with `make` the first time a tool asks for it.
    pub fn get_or_insert_with<T: 'static>(&mut self, make: impl FnOnce() -> T) -> &mut T {
        self.0
            .entry(std::any::TypeId::of::<T>())
            .or_insert_with(|| Box::new(make()))
            .downcast_mut()
            .expect("extension slot holds its own type")
    }
}

impl Ctx<'_> {
    fn step(&mut self, kind: &str) -> String {
        self.counter += 1;
        format!("{kind}-{}", self.counter)
    }
}

pub type ToolFn = Box<dyn Fn(&mut Ctx<'_>, &Value) -> Result<ToolOut, String>>;

#[derive(Default)]
pub struct Registry {
    tools: BTreeMap<String, ToolFn>,
}

impl Registry {
    pub fn register(&mut self, name: &str, f: ToolFn) {
        self.tools.insert(name.to_string(), f);
    }
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }
    /// The tools of order work with page readers: `put_parts`, `look`, `assign`, `write`, `compute`.
    pub fn order_tools() -> Registry {
        let mut r = Registry::default();
        r.register("put_parts", Box::new(tool_put_parts));
        r.register("look", Box::new(tool_look));
        r.register("assign", Box::new(tool_assign));
        r.register("write", Box::new(tool_write));
        r.register("compute", Box::new(tool_compute));
        r
    }
    fn run(&self, ctx: &mut Ctx<'_>, a: &Action) -> Result<ToolOut, String> {
        match self.tools.get(&a.tool) {
            Some(f) => f(ctx, &a.args),
            None => Err(format!("no tool named '{}'", a.tool)),
        }
    }
}

fn unit_of<'a>(ctx: &'a mut Ctx<'_>, args: &Value) -> Result<&'a mut UnitState, String> {
    let id = args["unit"].as_str().ok_or("'unit' is missing")?;
    ctx.units
        .get_mut(id)
        .ok_or_else(|| format!("no unit '{id}'"))
}

fn ui(id: &str) -> Target {
    Target::Ui(id.into())
}

fn tool_put_parts(ctx: &mut Ctx<'_>, _a: &Value) -> Result<ToolOut, String> {
    let order = ctx.manifest.order;
    let parts: Vec<Value> = ctx
        .manifest
        .units
        .iter()
        .map(|u| json!({ "no": u.no, "name": if u.name.is_empty() { &u.no } else { &u.name }, "shape": u.family, "qty": 1, "page": u.page }))
        .collect();
    let n = parts.len();
    let mut events = Vec::new();
    for u in &ctx.manifest.units {
        let s = ctx.step("part");
        events.push(
            Event::new(
                &s,
                "type",
                ui(&format!("part-{}-name", u.no)),
                &format!("add part {} ({}) from page {}", u.no, u.family, u.page),
            )
            .value(json!(u.family))
            .reason_key(
                "op.reason.add_part",
                json!({ "part": u.no, "family": u.family, "page": u.page }),
            ),
        );
    }
    // the server stores these events in the transaction of the parts edit
    let event_json = tool_events(events, "put_parts");
    call_ok(
        ctx.hands,
        "order_put_parts",
        json!({ "order": order, "parts": parts, "events": event_json, "reason": "the job sheet names these parts and the page of each" }),
    )?;
    Ok(ToolOut {
        events: vec![],
        result: json!({ "parts": n }),
    })
}

/// The events of a tool call that writes, as the JSON the server stores with the write.
fn tool_events(events: Vec<Event>, tool: &str) -> Vec<Value> {
    events
        .into_iter()
        .map(|mut e| {
            e.tool = tool.into();
            e.to_json()
        })
        .collect()
}

fn tool_look(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let page = a["page"].as_i64().ok_or("'page' is missing")?;
    let order = ctx.manifest.order;
    if !ctx.pages.contains_key(&page) {
        // the server's own readings of the page; if it has none yet, it reads the page now
        let mut r = call_ok(
            ctx.hands,
            "order_page_readings",
            json!({ "order": order, "page": page }),
        )?;
        if r["readings"].as_array().is_none_or(|a| a.is_empty()) {
            // the read writes readings and the page's read state, so it carries its own look event
            let fallback = r["reader"]
                .as_str()
                .unwrap_or("the server's reader")
                .to_string();
            let rs = ctx.step("look");
            let read_event = Event::new(
                &format!("{rs}r"),
                "look",
                Target::Page { page, bbox: None },
                &format!("reading page {page} with the server's reader"),
            )
            .reason_key("op.reason.look", json!({ "page": page, "name": fallback }));
            call_ok(
                ctx.hands,
                "order_read_page",
                json!({ "order": order, "page": page, "events": tool_events(vec![read_event], "look") }),
            )?;
            r = call_ok(
                ctx.hands,
                "order_page_readings",
                json!({ "order": order, "page": page }),
            )?;
        }
        ctx.pages.insert(page, PageRead::from_server(page, &r));
    }
    let n = ctx.pages[&page].readings.len();
    let name = ctx.pages[&page].reader.clone();
    let s = ctx.step("look");
    Ok(ToolOut {
        events: vec![
            Event::new(
                &s,
                "look",
                Target::Page { page, bbox: None },
                &format!("reading page {page} with {name}"),
            )
            .value(json!({ "numbers": n }))
            .reason_key("op.reason.look", json!({ "page": page, "name": name })),
        ],
        result: json!({ "page": page, "numbers": n }),
    })
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn tool_assign(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let spec = unit_of(ctx, a)?.spec.clone();
    let read = ctx.pages.get(&spec.page).ok_or("look at the page first")?;
    let rows = layout::table_rows(&read.readings, read.width, read.height);
    let bom = layout::bom_rows(&read.readings, &rows, &spec.parts);
    let mut readings = read.readings.clone();
    for row in &rows {
        for &i in row {
            readings[i].in_table = true;
        }
    }
    let unit = assign::family_unit(&spec.id, &spec.family, bom.clone())
        .ok_or_else(|| format!("the family '{}' has no slot set", spec.family))?;
    let asg = assign::assign(&unit, &readings, &FamilyVerifier);
    let mut events = Vec::new();
    for (row, b) in rows.iter().zip(&bom) {
        let cell = &readings[row[row.len() - 2]];
        let s = ctx.step("list");
        events.push(
            Event::new(
                &s,
                "point",
                Target::page_box(spec.page, cell.bbox),
                &format!(
                    "the material list gives blank {} x {} for {}",
                    fmt(b.width),
                    fmt(b.height),
                    b.part
                ),
            )
            .value(json!([b.width, b.height]))
            .reason_key(
                "op.reason.material_list",
                json!({ "width": fmt(b.width), "height": fmt(b.height), "part": b.part }),
            )
            .confidence(cell.confidence),
        );
    }
    for f in &asg.fills {
        let s = ctx.step("slot");
        let (how, reason_key, reason_args) = match f.status {
            Status::Confirmed => (
                "the only assignment that reproduces the material list".to_string(),
                "op.reason.slot_confirmed",
                json!({ "text": f.text, "value": fmt(f.value), "key": f.key }),
            ),
            Status::Proposed => (
                format!(
                    "the only assignment that reproduces the list, but the reading is flagged ({}): a person must check it",
                    f.flags.join(", ")
                ),
                "op.reason.slot_proposed",
                json!({ "text": f.text, "value": fmt(f.value), "key": f.key, "flags": f.flags.join(", ") }),
            ),
        };
        let reason = format!("'{}' is {} for {}: {how}", f.text, fmt(f.value), f.key);
        events.push(
            Event::new(
                &format!("{s}p"),
                "point",
                Target::page_box(f.page, f.bbox),
                &reason,
            )
            .value(json!(f.text))
            .reason_key(reason_key, reason_args.clone())
            .confidence(f.confidence)
            .flags(&f.flags),
        );
        events.push(
            Event::new(
                &format!("{s}c"),
                "click",
                Target::page_box(f.page, f.bbox),
                &reason,
            )
            .value(json!(f.key))
            .reason_key(reason_key, reason_args)
            .confidence(f.confidence)
            .flags(&f.flags),
        );
    }
    if let Some(q) = &asg.question {
        let s = ctx.step("ask");
        events.push(
            Event::new(&s, "ask", ui(&format!("question-{}", spec.no)), &q.en)
                .value(json!({ "code": q.code, "ja": q.ja }))
                .reason_key(
                    "op.reason.question",
                    json!({ "text": q.en, "code": q.code, "ja": q.ja }),
                ),
        );
    }
    let result = json!({
        "unit": spec.id, "fills": asg.fills.len(), "tried": asg.tried,
        "confirmed": asg.fills.iter().filter(|f| f.status == Status::Confirmed).count(),
        "question": asg.question.as_ref().map(|q| q.code),
    });
    unit_of(ctx, a)?.assignment = Some(asg);
    Ok(ToolOut { events, result })
}

fn tool_write(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let order = ctx.manifest.order;
    let (spec, asg) = {
        let u = unit_of(ctx, a)?;
        (
            u.spec.clone(),
            u.assignment.clone().ok_or("assign the unit first")?,
        )
    };
    let mut values = Vec::new();
    let mut confirm_values = Vec::new();
    let mut shown = Vec::new();
    let mut confirm_shown = Vec::new();
    for f in &asg.fills {
        // The server lets the operator confirm only a clean reading of the page: a plain number (source "read"),
        // no flags, confidence at or above the server's threshold. Anything else is proposed for a person.
        // The reading attachment is server-owned: every proposal names its reading, and only a later write
        // confirms the value from that same reading.
        let plain = assign::mark_of(&f.text) == assign::Mark::Plain;
        let reading_id = ctx
            .pages
            .get(&f.page)
            .and_then(|p| p.ids.get(f.reading))
            .copied();
        let confirm =
            f.status == Status::Confirmed && f.flags.is_empty() && plain && reading_id.is_some();
        let source = if plain { "read" } else { "derived" };
        let (note, reason_key, reason_args) = if confirm {
            (
                format!(
                    "read '{}' on page {}; the only assignment that matches the material list",
                    f.text, f.page
                ),
                "op.reason.read",
                json!({ "text": f.text, "page": f.page }),
            )
        } else {
            (
                format!(
                    "proposed from '{}' on page {}: {}, a person must check it",
                    f.text,
                    f.page,
                    if f.flags.is_empty() {
                        "not a clean page reading".to_string()
                    } else {
                        format!("flagged ({})", f.flags.join(", "))
                    }
                ),
                "op.reason.proposed",
                json!({ "text": f.text, "page": f.page, "flags": f.flags.join(", ") }),
            )
        };
        let mut value = json!({ "part": spec.no, "key": f.key, "value": f.value, "text": f.text,
            "bbox": f.bbox.map(f64::from), "source": source, "flags": f.flags,
            "confidence": f.confidence, "confirm": false, "note": note });
        // Only a plain reading is the written number itself; a marked reading (e.g. a diameter) is scaled, so its
        // stored number is not the value written and cannot be attached to it.
        if plain && let Some(id) = reading_id {
            value["reading_id"] = json!(id);
        }
        values.push(value);
        shown.push((
            f.key.clone(),
            f.value,
            f.confidence,
            f.flags.clone(),
            note.clone(),
            reason_key,
            reason_args.clone(),
        ));
        if confirm {
            confirm_values.push(json!({ "part": spec.no, "key": f.key, "value": f.value,
                "text": f.text, "bbox": f.bbox.map(f64::from), "source": source,
                "flags": f.flags, "confidence": f.confidence, "confirm": true,
                "reading_id": reading_id, "note": note }));
            confirm_shown.push((
                f.key.clone(),
                f.value,
                f.confidence,
                f.flags.clone(),
                note,
                reason_args,
            ));
        }
    }
    for (k, v) in &asg.fixed {
        values.push(json!({ "part": spec.no, "key": k, "value": v, "text": "", "source": "fixed", "flags": [], "confidence": 1.0, "confirm": false,
            "note": format!("set by rule, not on the drawing: {k} = {}", fmt(*v)) }));
    }
    if values.is_empty() {
        return Ok(ToolOut {
            events: vec![],
            result: json!({ "written": 0 }),
        });
    }
    let n = values.len();
    let mut events = Vec::new();
    for (key, value, conf, flags, note, reason_key, reason_args) in shown {
        let s = ctx.step("type");
        events.push(
            Event::new(&s, "type", ui(&format!("part-{}-{}", spec.no, key)), &note)
                .value(json!(value))
                .reason_key(reason_key, reason_args)
                .confidence(conf)
                .flags(&flags)
                .written(&spec.no, &key),
        );
    }
    for (k, v) in &asg.fixed {
        let s = ctx.step("type");
        let note = format!("set by rule, not on the drawing: {k} = {}", fmt(*v));
        events.push(
            Event::new(&s, "type", ui(&format!("part-{}-{}", spec.no, k)), &note)
                .value(json!(v))
                .reason_key("op.reason.fixed", json!({ "key": k, "value": fmt(*v) }))
                .written(&spec.no, k),
        );
    }
    // The server writes values and these events in one transaction; returning no events avoids a second post.
    let event_json = tool_events(events, "write");
    call_ok(
        ctx.hands,
        "order_put_values",
        json!({ "order": order, "values": values, "events": event_json }),
    )?;
    // The attachment is made by the proposal above. Only now can this value be confirmed from that reading;
    // a caller cannot attach it directly by sending `confirm: true` on its first write.
    if !confirm_values.is_empty() {
        let mut confirm_events = Vec::new();
        for (key, value, conf, flags, note, reason_args) in confirm_shown {
            let s = ctx.step("confirm");
            confirm_events.push(
                Event::new(
                    &s,
                    "confirm",
                    ui(&format!("part-{}-{}", spec.no, key)),
                    &note,
                )
                .value(json!(value))
                .reason_key("op.reason.read", reason_args)
                .confidence(conf)
                .flags(&flags)
                .written(&spec.no, &key),
            );
        }
        call_ok(
            ctx.hands,
            "order_put_values",
            json!({ "order": order, "values": confirm_values, "events": tool_events(confirm_events, "write") }),
        )?;
    }
    unit_of(ctx, a)?.written = true;
    Ok(ToolOut {
        events: vec![],
        result: json!({ "written": n }),
    })
}

fn tool_compute(ctx: &mut Ctx<'_>, a: &Value) -> Result<ToolOut, String> {
    let (project, order, order_compute) = (
        ctx.manifest.project,
        ctx.manifest.order,
        ctx.manifest.order_compute,
    );
    let (spec, asg) = {
        let u = unit_of(ctx, a)?;
        (
            u.spec.clone(),
            u.assignment.clone().ok_or("assign the unit first")?,
        )
    };
    let all_confirmed = !asg.fills.is_empty()
        && asg
            .fills
            .iter()
            .all(|f| f.status == Status::Confirmed && f.flags.is_empty())
        && asg.question.is_none();
    let mut events = Vec::new();
    if !all_confirmed {
        let s = ctx.step("ask");
        events.push(
            Event::new(
                &s,
                "ask",
                ui(&format!("question-{}", spec.no)),
                "not computed: every number must be confirmed first",
            )
            .value(json!({ "code": "not_confirmed" }))
            .reason_key("op.reason.not_computed", json!({})),
        );
        return Ok(ToolOut {
            events,
            result: json!({ "computed": false }),
        });
    }
    let mut params = serde_json::Map::new();
    for f in &asg.fills {
        params.insert(f.key.clone(), json!(f.value));
    }
    for (k, v) in &asg.fixed {
        params.insert(k.clone(), json!(v));
    }
    // derived values were not read; they are part of the verified request
    let unit = assign::family_unit(&spec.id, &spec.family, vec![]).ok_or("family")?;
    for (k, from, f) in &unit.derived {
        if let Some(v) = params.get(from).and_then(Value::as_f64) {
            params.insert(k.clone(), json!(v * f));
        }
    }
    if matches!(spec.family.as_str(), "branch_stub" | "header_with_branch") {
        params.insert("surface_reference".into(), json!("unknown"));
    }
    let s = ctx.step("compute");
    events.push(
        Event::new(
            &format!("{s}c"),
            "click",
            ui("compute-btn"),
            &format!("calculate {} with the confirmed numbers", spec.family),
        )
        .reason_key("op.reason.calculate", json!({ "family": spec.family })),
    );
    let r = call_ok(
        ctx.hands,
        "calculate",
        json!({ "project": project, "family": spec.family, "params": Value::Object(params) }),
    )?;
    let job = r["id"].as_i64().ok_or("the calculation gave no job id")?;
    let state = r["state"].as_str().unwrap_or("").to_string();
    let amoeba = call_ok(ctx.hands, "get_amoeba", json!({ "job": job }))?;
    let (mut held, mut served) = (Vec::new(), Vec::new());
    for p in &spec.parts {
        let d = call_ok(
            ctx.hands,
            "get_dxf_digest",
            json!({ "job": job, "part": p }),
        )?;
        if d["held"].as_bool().unwrap_or(false) {
            let why: String = d["hold"].to_string().chars().take(200).collect();
            let s2 = ctx.step("ask");
            events.push(
                Event::new(
                    &s2,
                    "ask",
                    ui(&format!("hold-{}-{}", spec.no, p)),
                    &format!("part {p} is held by the AMOEBA gate and gets no DXF: {why}"),
                )
                .value(json!({ "code": "held", "part": p }))
                .reason_key("op.reason.held", json!({ "part": p, "text": why })),
            );
            held.push(p.clone());
        } else {
            served.push(p.clone());
        }
    }
    events.push(
        Event::new(
            &format!("{s}r"),
            "compute",
            ui("compute-btn"),
            &format!("job {job} is {state}; the operator does not approve or release"),
        )
        .value(json!({ "job": job, "state": state, "held": held, "gate": amoeba["gate"] }))
        .reason_key("op.reason.job_state", json!({ "job": job, "state": state })),
    );
    if order_compute {
        let ev = Event::new(
            &format!("{s}o"),
            "compute",
            ui("order-compute-btn"),
            "compute the order with the confirmed values",
        )
        .reason_key("op.reason.order_compute", json!({}));
        call_ok(
            ctx.hands,
            "order_compute",
            json!({ "order": order, "events": tool_events(vec![ev], "compute"), "reason": "compute the order with the confirmed values" }),
        )?;
    }
    let u = unit_of(ctx, a)?;
    u.job = Some(job);
    u.job_state = Some(state.clone());
    u.held = held.clone();
    u.dxf_served = served;
    Ok(ToolOut {
        events,
        result: json!({ "computed": true, "job": job, "state": state, "held": held }),
    })
}

// ---------------------------------------------------------------- the deterministic planner

/// A fixed plan over the job sheet that calls the tools a model would call. Skips what the last result shows is
/// pointless (no fills: nothing to write or compute).
pub struct RulePlanner {
    queue: std::collections::VecDeque<Action>,
    unit: Vec<(String, i64)>,
    i: usize,
    stage: u8,
    started: bool,
}

impl RulePlanner {
    pub fn new(m: &Manifest) -> RulePlanner {
        RulePlanner {
            queue: Default::default(),
            unit: m.units.iter().map(|u| (u.id.clone(), u.page)).collect(),
            i: 0,
            stage: 0,
            started: false,
        }
    }
}

impl Brain for RulePlanner {
    fn name(&self) -> String {
        "rule planner".into()
    }
    fn next(&mut self, last: Option<Seen<'_>>) -> Option<Action> {
        if !self.started {
            self.started = true;
            return Some(Action::new("put_parts", json!({})));
        }
        if let Some(l) = &last {
            let ok = !l.failed;
            match (l.tool, ok) {
                ("assign", true) if l.result["fills"].as_i64().unwrap_or(0) == 0 => self.stage = 4,
                ("write", true) if l.result["written"].as_i64().unwrap_or(0) == 0 => self.stage = 4,
                ("assign" | "look" | "write", false) => self.stage = 4,
                _ => {}
            }
        }
        if let Some(a) = self.queue.pop_front() {
            return Some(a);
        }
        // advance: stage 0 look, 1 assign, 2 write, 3 compute, 4 next unit
        loop {
            let (id, page) = self.unit.get(self.i)?.clone();
            let a = match self.stage {
                0 => Action::new("look", json!({ "page": page })),
                1 => Action::new("assign", json!({ "unit": id })),
                2 => Action::new("write", json!({ "unit": id })),
                3 => {
                    self.stage = 0;
                    self.i += 1;
                    return Some(Action::new("compute", json!({ "unit": id })));
                }
                _ => {
                    self.stage = 0;
                    self.i += 1;
                    continue;
                }
            };
            self.stage += 1;
            return Some(a);
        }
    }
}

// ---------------------------------------------------------------- the loop

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status2 {
    Done,
    /// a person paused the operator
    Paused,
}

pub struct RunReport {
    pub status: Status2,
    pub actions: usize,
    pub events: usize,
    pub errors: Vec<String>,
    pub units: Vec<UnitState>,
}

fn paused(hands: &mut dyn Hands, order: i64, after: i64) -> Result<bool, String> {
    let r = call_ok(
        hands,
        "order_poll_events",
        json!({ "order": order, "after": after }),
    )?;
    Ok(r["paused"].as_bool().unwrap_or(false))
}

pub fn run(
    hands: &mut dyn Hands,
    brain: &mut dyn Brain,
    registry: &Registry,
    manifest: &Manifest,
) -> Result<RunReport, String> {
    let units = manifest
        .units
        .iter()
        .map(|u| {
            (
                u.id.clone(),
                UnitState {
                    spec: u.clone(),
                    assignment: None,
                    job: None,
                    job_state: None,
                    held: vec![],
                    dxf_served: vec![],
                    written: false,
                },
            )
        })
        .collect();
    let mut ctx = Ctx {
        hands,
        manifest,
        pages: BTreeMap::new(),
        units,
        ext: Extensions::default(),
        counter: 0,
    };
    let (mut actions, mut sent, mut errors) = (0usize, 0usize, Vec::new());
    let mut last: Option<(String, Value, bool)> = None;
    let status = loop {
        if paused(ctx.hands, manifest.order, 0)? {
            break Status2::Paused;
        }
        let seen = last.as_ref().map(|(t, r, f)| Seen {
            tool: t,
            result: r,
            failed: *f,
        });
        let Some(action) = brain.next(seen) else {
            break Status2::Done;
        };
        actions += 1;
        let (events, result, failed) = match registry.run(&mut ctx, &action) {
            Ok(o) => (o.events, o.result, false),
            Err(e) => {
                let s = ctx.step("error");
                errors.push(format!("{}: {e}", action.tool));
                let msg: String = e.chars().take(280).collect();
                let ev = Event::new(&s, "ask", ui("operator-error"), &msg)
                    .value(json!({ "code": "tool_error", "tool": action.tool }))
                    .reason_key("op.reason.tool_error", json!({ "text": msg }));
                (vec![ev], json!({ "error": e }), true)
            }
        };
        if !events.is_empty() {
            let batch: Vec<Value> = events
                .into_iter()
                .map(|mut e| {
                    e.tool = action.tool.clone();
                    e.to_json()
                })
                .collect();
            let n = batch.len();
            match call_ok(
                ctx.hands,
                "order_post_events",
                json!({ "order": manifest.order, "events": batch }),
            ) {
                Ok(_) => sent += n,
                Err(e) if e.contains("operator_paused") => break Status2::Paused,
                Err(e) => return Err(e),
            }
        }
        last = Some((action.tool.clone(), result, failed));
    };
    Ok(RunReport {
        status,
        actions,
        events: sent,
        errors,
        units: ctx.units.into_values().collect(),
    })
}

#[cfg(test)]
mod ext_tests {
    use super::Extensions;

    #[derive(Default, Debug, PartialEq)]
    struct CadCanvas {
        entities: Vec<String>,
    }

    #[test]
    fn a_tool_keeps_its_own_state_without_touching_the_loop() {
        let mut ext = Extensions::default();
        assert!(ext.get::<CadCanvas>().is_none());
        ext.get_or_insert_with(CadCanvas::default)
            .entities
            .push("line-1".into());
        ext.get_or_insert_with(CadCanvas::default)
            .entities
            .push("line-2".into());
        assert_eq!(ext.get::<CadCanvas>().unwrap().entities.len(), 2);
        // another type is a separate slot
        assert_eq!(ext.insert(7u32), None);
        assert_eq!(ext.insert(8u32), Some(7));
        assert_eq!(ext.get::<u32>(), Some(&8));
        ext.get_mut::<CadCanvas>().unwrap().entities.clear();
        assert!(ext.get::<CadCanvas>().unwrap().entities.is_empty());
    }
}
