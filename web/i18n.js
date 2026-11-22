// NEXUS FABRIC message catalogue: English and Japanese. One module, no dependencies, no external resources.
//
// Two kinds of text reach the screen:
//  1. Text the CLIENT owns (navigation, labels, messages, state names ...): every string is a key in EN and JA
//     below. Messages are whole sentences with {placeholders}; nothing is concatenated from fragments, so
//     Japanese word order is free. `**bold**` and `` `code` `` are the only markup a message may carry.
//  2. Text the ENGINE or the SERVER writes (gate details, check methods, assumptions, refusal reasons ...).
//     It arrives in English and the server cannot be asked for another language. Stable identifiers get a
//     reviewed Japanese label keyed by the identifier (ID_JA). Prose is translated ONLY when every sentence
//     matches a known template exactly; numbers, units and identifiers are copied through as received.
//     Anything else is shown in the original English, marked as engine output. Nothing is ever translated
//     in part. docs/design/LOCALISATION.md lists which texts fall in which class.
//
// Japanese terminology follows the trade's own words (板展開, 取合い, けがき面, 母管, 枝管 ...) and the product's English
// terms. It has NOT been reviewed by a native-speaking fabrication engineer; the glossary and its confidence are in
// docs/design/LOCALISATION.md.

export const LANGS = ['en', 'ja'];
export const LANG_NAME = { en: 'English', ja: '日本語' };

let cur = 'en';
// Keys that were asked for and are missing in the active language (the English text is shown instead).
// The browser test fails if this is ever non-empty.
export const missing = new Set();

export const lang = () => cur;
export function setLang(l) {
  cur = LANGS.includes(l) ? l : 'en';
  document.documentElement.lang = cur;
  return cur;
}
// Saved choice, else Japanese if the browser asks for it, else English. `?lang=ja|en` overrides both
// for this page load only (tests, screen capture, links sent to a colleague).
export function initialLang(saved) {
  let q = null;
  try { q = new URLSearchParams(location.search).get('lang'); } catch { /* no URL API: ignore */ }
  if (LANGS.includes(q)) return q;
  if (LANGS.includes(saved)) return saved;
  const want = (navigator.languages && navigator.languages[0]) || navigator.language || '';
  return /^ja\b/i.test(want) ? 'ja' : 'en';
}

const escHtml = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fill = (s, p) => (p ? s.replace(/\{(\w+)\}/g, (x, k) => (k in p ? p[k] : x)) : s);
function pick(key, p) {
  let m = MSG[cur][key];
  if (m === undefined) { missing.add(`${cur}:${key}`); m = MSG.en[key]; }
  if (m === undefined) return key;
  if (Array.isArray(m)) m = m[p && Number(p.n) === 1 ? 0 : 1]; // [singular, plural]; Japanese never needs it
  return m;
}
// Plain text. Use for textContent, attribute values (escape them) and anything that is not HTML.
export const t = (key, p) => fill(pick(key, p), p);
// HTML. The message itself is escaped, then **bold** and `code` become markup, then the placeholders are
// filled with what the caller passes: the caller escapes values that come from users or the server.
export const h = (key, p) => fill(escHtml(pick(key, p)).replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`(.+?)`/g, '<span class="mono">$1</span>'), p);
export const has = (key) => MSG[cur][key] !== undefined || MSG.en[key] !== undefined;

// ---------------------------------------------------------------- English (the reference; every key must exist in JA)
const EN = {
  // build status: on screen at all times
  'build.short': 'Development build — not validated',
  'build.long': ' — outputs must not be used for manufacturing',
  'build.full': 'Development build — not validated — outputs must not be used for manufacturing',
  'build.status': 'DEVELOPMENT BUILD · NOT VALIDATED · NOT FOR MANUFACTURING',
  'build.login': '**Development build.** Not validated. No output may be used for manufacturing.',

  // states and verdicts (the client's fixed vocabulary)
  'state.APPROVED': 'Approved', 'state.REJECTED': 'Rejected', 'state.SUPERSEDED': 'Superseded', 'state.NO_RESULT': 'No result',
  'state.REVIEW_REQUIRED': 'REVIEW REQUIRED', 'state.REFUSED': 'REFUSED',
  'result.PASS': 'PASS', 'result.FAIL': 'FAIL', 'result.INCONCLUSIVE': 'INCONCLUSIVE', 'result.NOT_IMPLEMENTED': 'NOT IMPLEMENTED', 'result.NOT_RUN': 'NOT RUN',
  'obl.open': 'open', 'obl.discharged': 'discharged', 'obl.discharged_by_decision': 'discharged by decision', 'obl.waived': 'waived',
  'adv.open': 'open', 'adv.acknowledged': 'acknowledged',
  'dec.confirmed': 'confirmed', 'dec.waived': 'waived',
  'role.admin': 'admin', 'role.engineer': 'engineer', 'role.viewer': 'viewer',
  'planned.tag': 'Planned', 'planned.sub': 'not implemented in this build', 'planned.aria': 'Planned: {title}',
  'yes': 'yes', 'no': 'no', 'retry': 'Retry', 'cancel': 'Cancel', 'close': 'Close', 'creating': 'Creating…', 'storing': 'Storing…',
  'job.n': 'Job #{id}', 'job.n.lc': 'job #{id}', 'project.n': 'Project {id}', 'project.tip': 'Project #{id}',
  'unit.deg': 'deg',

  // engine and server text shown as received
  'eng.tag': 'engine output', 'srv.tag': 'server message',
  'engsrc.aria': 'Engine text: translation or original', 'engsrc.label': 'Engine text', 'engsrc.ja': 'Japanese', 'engsrc.en': 'Original',
  'engsrc.tip': 'Engine and server text: reviewed translation, or the original English',

  // shell
  'shell.skip': 'Skip to content', 'shell.nav.main': 'Main', 'shell.crumbs': 'Breadcrumb',
  'shell.rail.toggle': 'Toggle navigation labels', 'shell.menu': 'Navigation menu', 'shell.rail.tip': 'Navigation  [', 'shell.brand.aria': 'NEXUS FABRIC, dashboard',
  'shell.lang.aria': 'Language', 'shell.lang.tip': 'Language  L',
  'shell.bell': 'Notifications', 'shell.bell.new': 'Notifications, {n} new',
  'shell.help.aria': 'Keyboard shortcuts and help', 'shell.help.tip': 'Shortcuts  ?',
  'shell.theme.aria': 'Switch between dark and light theme', 'shell.theme.tip': 'Theme  T',
  'shell.account.aria': 'Account: {user}, role {role}', 'shell.signout': 'Sign out', 'shell.progress': 'Request in progress',
  'shell.conn.ok': 'Server connected', 'shell.conn.off': 'Server unreachable', 'shell.engine': 'Engine {id}',
  'shell.obl': ['{n} open scrutiny obligation', '{n} open scrutiny obligations'], 'shell.obl.aria': '{n} open obligations', 'shell.shortcuts': 'Shortcuts',
  'notes.title': 'Notifications', 'notes.close': 'Close notifications', 'notes.empty.t': 'Nothing yet', 'notes.empty.p': 'Results of actions you take in this session appear here.',
  'notes.foot': 'This browser session only. A server-side notification service and job queue are planned, not implemented: jobs run synchronously.',
  'notes.k.err': 'error', 'notes.k.ok': 'done', 'notes.k.warn': 'attention', 'notes.k.info': 'info', 'toast.dismiss': 'Dismiss notification',

  // navigation
  'nav.g.overview': 'Overview', 'nav.g.engineering': 'Engineering', 'nav.g.assurance': 'Assurance', 'nav.g.system': 'System',
  'nav.dashboard': 'Dashboard', 'nav.projects': 'Projects & Documents', 'nav.workspace': 'Engineering Workspace', 'nav.manufacturing': 'Manufacturing',
  'nav.amoeba': 'AMOEBA Control Centre', 'nav.verification': 'Verification & Quality', 'nav.admin': 'Administration', 'nav.account': 'Account',

  // shortcut overlay
  'help.title': 'Keyboard shortcuts', 'help.list': 'Shortcut list', 'help.goto': 'Go to', 'help.anywhere': 'Anywhere', 'help.viewer': 'Pattern viewer', 'help.panels': 'Panels',
  'help.k.overlay': 'This overlay', 'help.k.new': 'New calculation', 'help.k.filter': 'Focus the filter of the current list', 'help.k.rail': 'Show or hide navigation labels',
  'help.k.theme': 'Dark or light theme', 'help.k.lang': 'English or Japanese', 'help.k.esc': 'Close overlay, leave a field',
  'help.v.fit': 'Fit pattern to view (also double-click)', 'help.v.zoom': 'Zoom in, out (wheel zooms at the cursor)', 'help.v.pan': 'Pan when the viewer has focus (drag pans)',
  'help.v.measure': 'Measure between two points; snaps to computed vertices', 'help.v.part': 'Switch part of an assembly', 'help.v.esc': 'Clear measurement, then selection',
  'help.p.tabs': 'On a tab row: previous, next tab', 'help.p.split': 'On a panel divider: resize the panel', 'help.p.curves': 'Curve table rows select the curve in the viewer',
  'help.foot': '{build}. Hover or focus any icon button to see its name and shortcut.',

  // sign in and first-run setup
  'login.doc': 'Sign in', 'login.h': 'Sign in', 'login.h.setup': 'Create the administrator',
  'login.sub': 'Flat patterns for pipe joints and plate parts. Results are not released for manufacturing.', 'login.sub.setup': 'First start of this installation. This account manages users and projects.',
  'login.user': 'Username', 'login.pass': 'Password', 'login.pass.setup': 'Password (at least 10 characters)', 'login.token': 'Setup token',
  'login.token.hint': 'Shown to the person who installed this server (file `setup-token` in the data directory, or the value of NF_SETUP_TOKEN). It can be used once.',
  'login.go': 'Sign in', 'login.go.setup': 'Create administrator', 'login.busy': 'Signing in…', 'login.busy.setup': 'Creating…',
  'login.legal': 'Runs entirely on this server. No external network resources are loaded.',
  // desktop mode, first-run screen (wording decided by the owner: docs/security/DESKTOP_KEY_ASSESSMENT.md section 6)
  'login.desktop.h': 'Desktop mode: how your data is kept',
  'login.desktop.p1': 'This program does not encrypt its database or the whole data folder. Stored files have their own encryption, but that does not make the data on this computer an encrypted data set.',
  'login.desktop.p2': 'How well the data is protected depends on this computer and how it is run. Recommended: disk encryption, an individual account for each person, access to the data folder only for the people who use this program, and protected backups.',
  'login.desktop.p3': 'Disk encryption protects a lost or stolen computer; it does not protect a copy of the data folder taken while the disk is unlocked. A computer account by itself does not encrypt the database.',
  'login.e.user': 'Enter your username.', 'login.e.pass': 'Enter your password.', 'login.e.len': 'Use at least 10 characters.', 'login.pw.more': '{n} more characters needed.', 'login.pw.ok': 'Meets the 10-character minimum. A longer passphrase of several words is stronger.', 'login.e.token': 'Enter the setup token of this installation.',
  'login.bad': 'Incorrect username or password.', 'login.ended': 'Your session has ended. Sign in again.', 'login.changed': 'Password changed. All sessions were signed out; sign in again.',
  'api.signin': 'Please sign in.', 'api.unreachable': 'Cannot reach the server. Check that it is running, then retry.', 'api.failed': 'Request failed ({status})',
  'start.unreachable': 'Cannot reach the server',

  // shared tables and empty states
  'th.job': 'Job', 'th.project': 'Project', 'th.family': 'Geometry family', 'th.parts': 'Parts', 'th.state': 'Release state', 'th.replay': 'Replay audit', 'th.entered': 'Entered',
  'th.what': 'What went wrong', 'th.fromjob': 'From job', 'th.matched': 'Matched since', 'th.recorded': 'Recorded',
  'th.record': 'Record', 'th.relation': 'Relation', 'th.dependson': 'Depends on', 'th.name': 'Name', 'th.docs': 'Documents', 'th.jobs': 'Jobs', 'th.created': 'Created',
  'jobs.none.t': 'No engineering jobs yet', 'jobs.none.cta': 'Create a project', 'jobs.none.p': 'Start a calculation from a project: enter the dimensions of a part. The engine computes its flat pattern, runs the checks and writes one DXF file per part.',
  'new.calc': 'New calculation', 'jobs.filter': 'Filter jobs', 'jobs.all': 'All jobs', 'to.dashboard': 'Dashboard',
  'parts.none': 'none — input refused', 'obl.open.n': ['{n} open obligation', '{n} open obligations'],
  'replay.match': 'Replay match', 'replay.mismatch': 'Replay mismatch', 'replay.none': 'not replayed',
  'lesson.corrected': '{fam} · {param}: {wrong} was wrong, {right} is right', 'lesson.other': '{kind} · {fam}',
  'lessons.none.t': 'No lessons recorded yet',
  'lessons.none.p': 'A lesson is recorded automatically when an engineer corrects a value, when verification rejects a result, or when a stored result fails its integrity check.',
  'flt.count': '{a} of {b}', 'flt.nomatch': 'Nothing matches “{q}”.', 'flt.clear': 'Clear filter',

  // dependency graph
  'dg.level0': 'Entered / declared', 'dg.level': 'Derived · level {n}',
  'dg.idle': 'Select a record to trace what it depends on {amber} and what a change to it reaches {blue}.', 'dg.amber': '(amber)', 'dg.blue': '(blue)',
  'dg.reach': ['{node} depends on {n} record. A change to it reaches {m}: {list}.', '{node} depends on {n} records. A change to it reaches {m}: {list}.'],
  'dg.reach.none': ['{node} depends on {n} record. A change to it reaches 0: nothing downstream.', '{node} depends on {n} records. A change to it reaches 0: nothing downstream.'],
  'dg.layers.aria': 'Dependency layers, {n} records and {e} edges', 'dg.node.aria': '{label}: depends on {a}, used by {b}',
  'dg.layers.note': 'Layers read top to bottom: entered values first, then what is derived from them.',
  'dg.graph.aria': 'Dependency graph, {n} records and {e} edges. Scrollable.', 'dg.flow.note': 'The flow reads left to right: entered values, then what is derived from them.',
  'dg.edges': 'Edge list ({n})',

  // dashboard
  'dash.sub': 'Engine {engine} · running for {min} min', 'dash.totals': 'Totals',
  'dash.s.projects': 'Projects', 'dash.s.jobs': 'Engineering jobs', 'dash.s.review': 'Review required', 'dash.s.obl': 'Open obligations',
  'dash.s.sup': 'Superseded', 'dash.s.none': 'No result (refused)', 'dash.s.rej': 'Rejected', 'dash.s.records': 'Stored records',
  'dash.recent': 'Recent engineering jobs', 'dash.needs': 'Needs an engineer', 'dash.needs.open': 'Open job #{id}', 'dash.needs.raised': 'raised {when}',
  'dash.needs.none.t': 'No open scrutiny obligations', 'dash.needs.none.p': 'Obligations are raised when a correction reaches other records.',
  'dash.state': 'State of all jobs', 'dash.bar.aria': '{review} review required, {sup} superseded, {none} no result, {rej} rejected, {ok} approved',
  'dash.l.review': '{n} review required', 'dash.l.sup': '{n} superseded', 'dash.l.none': '{n} no result · {m} rejected', 'dash.l.ok': '{n} approved',
  'dash.state.foot': 'No job can reach APPROVED in this build: the engineer approval gate (G16) is not implemented. “No result” means the engine refused the input and computed nothing.',
  'dash.partial': 'Counts cover the {n} most recent of {total} jobs (what the server lists).',
  'dash.planned.t': 'Processing queue and server notifications',
  'dash.planned.1': 'Jobs run synchronously in the request; there is no queue to monitor.', 'dash.planned.2': 'No notification service: the bell lists only what happened in this browser session.',

  // projects and one project
  'proj.crumb': 'Projects', 'proj.sub': 'Projects, their source documents and the snapshot hash recorded for each.', 'proj.panel': 'Projects', 'proj.filter': 'Filter projects',
  'proj.none.t': 'No projects yet', 'proj.none.viewer': 'An administrator has not added you to a project yet.', 'proj.none.p': 'Create the first project below. Jobs and documents belong to a project.',
  'proj.new': 'New project', 'proj.f.name': 'Name', 'proj.f.desc': 'Description', 'proj.create': 'Create project', 'proj.e.name': 'Give the project a name.', 'proj.created': 'Project #{id} created.',
  'proj.planned.t': 'Revisions and comparison', 'proj.planned.1': 'Drawing revision chains and differences between revisions', 'proj.planned.2': 'Comparison of two project snapshots',
  'project.meta': 'project #{id} · created by {by} · {when}', 'project.jobs': 'Engineering jobs', 'project.docs': 'Source documents', 'project.access': 'Access',
  'th.file': 'File', 'th.dtype': 'Detected type', 'th.size': 'Size (bytes)', 'th.sha': 'SHA-256', 'th.uploaded': 'Uploaded',
  'doc.mismatch': 'extension ≠ content', 'docs.none.t': 'No documents stored', 'docs.none.p': 'Stored files are hashed and encrypted. They are kept as evidence; their content is not read.',
  'doc.file.aria': 'File to store', 'doc.choose': 'Choose file…', 'doc.nofile': 'No file chosen', 'doc.store': 'Store file', 'doc.e.nofile': 'Choose a file first.',
  'doc.accepted': 'Accepted by content: PDF, PNG, JPEG, TIFF, DXF, STEP (max 50 MB). Files are hashed and stored encrypted. **No drawing content is read when a file is stored.** A PDF is read only when you convert it to DXF, by a separate isolated program.',
  'doc.snapshot': 'Current project snapshot: {id}', 'doc.snapshot.none': 'none', 'doc.stored': 'Stored ({type}). {interp}',
  'conv.view_side': 'Viewing side', 'conv.plate_axes': 'Plate axes', 'conv.rolling_axis': 'Rolling axis', 'conv.seam_edges': 'Seam edges', 'conv.end_edges': 'End edges', 'conv.angle_datum': 'Angular datum', 'conv.origin': 'Plate origin', 'conv.kerf': 'Kerf', 'conv.status': 'Status', 'conv.offset_delta': 'Offset', 'conv.offset_sign': 'Offset sign', 'conv.branch_angle': 'Branch angle',
  'conv.action': 'Convert to DXF', 'conv.action.aria': 'Convert {file} to DXF', 'conv.label': 'Converted from PDF - not verified - not released', 'conv.panel': 'PDF to DXF conversions', 'conv.th.doc': 'Source document', 'conv.th.page': 'Page', 'conv.th.result': 'Result', 'conv.title': 'Convert PDF to DXF', 'conv.rules': 'The converter reads the vector lines of one page. **A scale is required**: a PDF has no trustworthy real-world scale, so none is guessed. Scanned or raster drawings are refused. Dimension text is not read. The result is **converted from PDF - not verified - not released**: it is not a flat pattern and is never attached to a calculation.', 'conv.form': 'Page and scale', 'conv.f.page': 'Page', 'conv.f.kind': 'Scale given as', 'conv.f.kind.ratio': 'Drawing scale 1:N', 'conv.f.kind.mmpt': 'Millimetres per PDF point', 'conv.f.scale': 'Scale value (required)', 'conv.f.scale.hint': 'For a drawing scale of 1:10 enter 10. In millimetres per PDF point, 1:1 is 25.4/72 = 0.352778.', 'conv.f.tol': 'Chord tolerance for curves, mm (optional)', 'conv.f.tol.hint': 'Blank: the converter\'s default is used.', 'conv.f.fill': 'Include filled outlines', 'conv.f.text': 'Include text', 'conv.submit': 'Convert to DXF', 'conv.running': 'Converting…', 'conv.done': 'Conversion #{id} stored: converted from PDF - not verified - not released.', 'conv.refused': 'The converter refused the document: {code} (limit: {limit}). Nothing was converted.', 'conv.refused.rec': 'The refusal is recorded as conversion #{id}.', 'conv.e.scale': 'Enter a scale. A PDF has no trustworthy real-world scale, so none is guessed.', 'conv.e.nodoc': 'This document is not in the project.', 'conv.e.notpdf': 'Only a document detected as PDF can be converted.', 'conv.e.viewer': 'Your role may view conversions but not start one.', 'conv.view.title': 'Conversion #{id}', 'conv.view.sub': 'From source document #{doc}', 'conv.crumb.project': 'Project #{id}', 'conv.notflat': 'This drawing reproduces the lines of the PDF at the scale given. It is not a flat pattern, has not been checked against any requirement and is not attached to any calculation.', 'conv.r.title': 'Conversion report', 'conv.r.status': 'Result', 'conv.r.code': 'Code', 'conv.r.message': 'Message', 'conv.r.page': 'Page', 'conv.r.scale': 'Scale', 'conv.r.tol': 'Chord tolerance', 'conv.r.bbox': 'Bounding box', 'conv.r.source': 'Source SHA-256', 'conv.r.version': 'Converter version', 'conv.r.by': 'Converted by', 'conv.r.counts': 'Counts', 'conv.r.warnings': 'Warnings', 'conv.r.skipped': 'Text skipped, by reason', 'conv.dl.dxf': 'Download DXF', 'conv.dl.report': 'Download report', 'conv.preview': 'Preview of the converted file', 'conv.preview.note': 'Lines of the DXF only; text is not drawn. Not a flat pattern.', 'conv.preview.none': 'Nothing to preview.', 'conv.preview.aria': 'Line drawing of the converted DXF, {n} polylines', 'conv.scale.ratio': '1:{n}', 'conv.scale.mmpt': '{x} mm per PDF point', 'conv.st.refused': 'refused', 'conv.st.limit': 'limit exceeded',
  'member.remove': 'Remove', 'member.remove.aria': 'Remove {user} from the project', 'member.confirm': 'Confirm: remove {user}', 'members.none': 'No members.',
  'members.hint': 'Administrators see every project; other users only the projects they are members of.',
  'member.add.label': 'Add a user', 'member.all': 'Every active user is already a member', 'member.add': 'Add member', 'member.opt': '{user} — {role}',
  'member.added': '{user} added to the project.', 'member.removed': '{user} removed from the project.',

  // job browser
  'ws.sub': 'Open a job to inspect its flat pattern, verification, gates and provenance — or compute a new one.', 'ws.all': 'All engineering jobs',

  // new calculation
  'math.noproj.t': 'Create a project first', 'math.noproj.p': 'Each calculation is recorded in a project. Create a project first, then return to this form.', 'math.noproj.go': 'Go to projects',
  'math.viewer.t': 'Your role can view results but not run calculations', 'math.viewer.p': 'Ask an administrator for the engineer role.', 'math.viewer.go': 'Browse jobs',
  'math.meta': 'Surface development with independent checks. Parameters are entered manually.',
  'math.banner': '**PROVISIONAL MODELS** — radii are used as reference-surface radii exactly as entered. Thickness, allowances, weld gap and kerf are not modelled.',
  'math.inputs.aria': 'Inputs', 'math.pane.h': 'Inputs', 'math.sec.job': 'Job', 'math.f.project': 'Project', 'math.f.family': 'Geometry family', 'math.sec.params': 'Parameters',
  'math.tag.from': 'from job #{id}', 'math.tag.example': 'example values',
  'math.hint.from': 'Copied from the earlier job. Check every value.', 'math.hint.example': 'Pre-filled numbers are example values, not recommendations.',
  'math.run': 'Develop, verify and generate DXF', 'math.run.unusual': 'Run with these unusual values', 'math.busy': 'Computing and verifying…',
  'math.sref.label': 'Entered radii refer to which wall surface?',
  'sref.unknown': 'unknown — to be established', 'sref.outside': 'outside', 'sref.inside': 'inside', 'sref.neutral': 'neutral',
  'math.psref.same': 'Same as the surface above', 'math.psref.hint': 'Each part names the wall surface its contact radius refers to. "Same as the surface above" uses the one value above for that part, as before.',
  'math.joint.none': 'Not declared (single contact curve)', 'math.joint.set': 'Set-on: branch inside, header outside',
  'math.joint.hint': 'Leave this at "Not declared" unless the joint is set-on. Not declared computes one contact curve of the entered radii and does not call it set-on. Set-on needs the branch INSIDE surface on the header OUTSIDE surface; any other declaration is refused.',
  'joint.h': 'Joint, header and branch', 'joint.none.v': 'not declared', 'req.refused': 'refused',
  'v.joint.pair': 'A set-on joint needs the branch INSIDE surface on the header OUTSIDE surface. Now: header {hs}, branch {bs}.',
  'math.split.l': 'Resize inputs panel', 'math.split.r': 'Resize notes panel', 'math.stage.h': 'Parameter schematic', 'math.stage.tag': 'from the form · not a result',
  'math.limits.aria': 'Modelling limits', 'math.limits.h': 'Scope of this build', 'math.result.h': 'What the result will be',
  'math.result.1': 'A flat pattern per part with its curves, lengths and estimated numerical error.', 'math.result.2': 'Independent verification checks with primary and reference values.',
  'math.result.3': 'Release gates. Every job stays in **REVIEW REQUIRED** or **REJECTED**: engineer approval is not implemented.', 'math.result.4': 'DXF files named `NOT-RELEASED`.',
  'math.result.5': 'If the engine refuses the values, the job is recorded with **NO RESULT** and its reason; nothing is computed.',
  'math.planned.h': 'Planned', 'math.planned.tag': 'not implemented',
  'math.planned.1': 'Constraint solving from entered dimensions', 'math.planned.2': 'Assemblies other than one header with one branch; several branches; flanges and supports',
  'math.planned.3': 'Rectangular-to-round and irregular transitions; equal-diameter and partial branch intersections; oblique cylinders; near-cylindrical cones',
  'math.planned.4': 'Thickness / neutral-surface treatment, bend and roll allowances from calibrated process data', 'math.planned.5': 'Uncertainty and tolerance analysis; interval-certified error bounds',
  'math.planned.6': 'Plate division and seam optimisation',
  'math.conv.h': 'Sign and view conventions', 'math.conv.tag': 'provisional',
  'math.conv.lead': 'Conventions of this build (Spec 02 §4.4), stated by the client: the result data does not carry them. Provisional — not reviewed by a fabrication engineer.',
  'math.conv.1': 'The branch leans toward the higher station (+x) when β is below 90°.',
  'math.conv.2': 'A positive axis offset δ moves the branch axis away from the viewer of the side elevation, and to the right in the end view.',
  'math.conv.3': 'The end view is seen from the high-station end of the header, looking back toward station 0 (third-angle arrangement).',
  'math.conv.4': 'A wrong sign of δ gives a mirror-image assembly in which every check still passes.',
  'math.notsent': ['**Not sent.** {n} value cannot be computed; see the marked field.', '**Not sent.** {n} values cannot be computed; see the marked fields.'],
  'obl.onsup': 'on a superseded result',
  'obl.onsup.note': ['{n} open obligation is on a superseded result: it is listed, and it is not counted as open work.', '{n} open obligations are on superseded results: they are listed, and they are not counted as open work.'],
  'math.plaus.t': 'Unusual size — check the unit (mm).', 'math.plaus.p': 'Press the button again to run with these values.',
  'math.toast.none': 'Job #{id} recorded with NO RESULT — the engine refused the input: {why}', 'math.toast.ok': 'Job #{id} computed; its checks ran. State: {state}.',
  'math.why.default': 'no pattern produced', 'math.state.default': 'see gates',
  'v.number': '{label}: enter a number using digits and a decimal point, for example 1.5. A comma is not accepted.', 'v.below': '{label} = {v} mm is below 1e-6 mm, the smallest dimension this build has been exercised with. This is an implementation limit, not an engineering rule.', 'v.positive': '{label} must be greater than 0.', 'v.big': '{label} = {v} mm is more than 20 m.', 'v.small': '{label} = {v} mm is less than 1 mm.',
  'v.r1r2': 'Large-end radius must be greater than the small-end radius (large end first).', 'v.e': 'Offset of small end must be 0 or greater.',
  'v.beta': 'Angle between axes must be within 10° to 170° in this build.',
  'v.tangent': '|axis offset| + branch radius ({sum}) must be smaller than the header radius ({R}): tangent, partial and equal-diameter intersections are not implemented.',
  'v.station': 'Branch position must lie inside the header length.',
  'v.outside': '{d} ({dv}) cannot be larger than {c} ({cv}) when the contact radii are declared as outside: that would be a negative wall thickness.',
  'v.inside': '{d} ({dv}) cannot be smaller than {c} ({cv}) when the contact radii are declared as inside: that would be a negative wall thickness.',
  'v.neutral': '{d} ({dv}) must equal {c} ({cv}) when the contact radii are declared as neutral.',

  // parameter schematic
  'sk.msg': 'Enter a positive number in every field to see the schematic.',
  'sk.cap': 'Drawn to scale from the values in the form, in mm. An aid for reading the parameters: not the computed pattern, not verified.',
  'sk.notdrawn': 'Not drawn: {items}.', 'sk.dev': '{label} {v} mm → flat width 2π·r = {w} mm', 'sk.aria': 'Schematic of the entered {fam} dimensions: {dims}',
  'sk.d.length': 'length', 'sk.d.hlen': 'header length', 'sk.d.station': 'station',
  'sk.v.sidev': 'Side view', 'sk.v.endv': 'End view', 'sk.v.cone': 'Side elevation, large end at the bottom',
  'sk.v.side': 'Side elevation', 'sk.v.side.part': 'Side elevation (header shown in part)', 'sk.v.end': 'End view from the high-station end', 'sk.v.end.part': 'End view along the header axis',
  'sk.ax.away': '+y, +δ: away from you', 'sk.ax.toward': '+x: toward you', 'sk.ax.station': 'x, station', 'sk.ax.x': 'x',
  'sk.delta.pos': 'δ {v}: branch axis displaced away from the viewer of the side elevation', 'sk.delta.neg': 'δ {v}: branch axis displaced toward the viewer of the side elevation', 'sk.delta.zero': 'δ 0: the two axes intersect',

  // one job: header, banner, panes
  'calc.notfound.title': 'Job not found', 'calc.notfound.crumb': 'Not found', 'calc.notfound.t': 'Job #{id} does not exist',
  'calc.notfound.p': 'It was never created, or it belongs to a project you are not a member of.',
  'job.corrects': 'corrects #{id}', 'job.enteredby': 'entered by {by}',
  'job.successor': 'Open current job #{id}', 'job.correct': 'Record correction…', 'job.replay': 'Run replay audit', 'job.replay.tip': 'Recompute in a separate process and compare the closure hash',
  'job.clone': 'New from these values', 'job.clone.tip': 'New calculation starting from these values',
  'ban.sup': '**SUPERSEDED by {job}** after a correction. This result must not be used; its files are withheld. It is kept only as a record.',
  'ban.none': '**NO RESULT** — the engine refused this input: {why}. Nothing was computed; there is nothing to review, correct or manufacture.',
  'ban.rej': '**REJECTED** — a critical gate failed. No output of this job may be used.',
  'ban.review': '**REVIEW REQUIRED** — this result cannot be released for manufacturing. The Gates tab gives every reason.', 'ban.review.sum': 'Not passed: {list} ({n} of {total} gates). Each reason is in the Gates tab.', 'ban.none.min': 'Threshold: `s_end` must be greater than {n} mm for this geometry (calculated by the engine).',
  'ban.none.default': 'no pattern was produced',
  'pane.props.aria': 'Inputs and properties', 'pane.props.h': 'Properties', 'pane.results.aria': 'Results, verification and gates', 'split.props': 'Resize properties panel', 'split.results': 'Resize results panel',
  'req.h': 'Request', 'req.tag': 'entered manually', 'req.correct': 'Correct', 'req.correct.aria': 'Correct {label}', 'req.hint': 'Not linked to a source drawing.',
  'sel.h': 'Selection', 'curves.h': 'Curves', 'quant.h': 'Pattern quantities', 'assume.h': 'Assumptions',
  'tabs.aria': 'Results', 'tab.changes': 'Changes', 'tab.gates': 'Gates', 'tab.checks': 'Checks', 'tab.scrutiny': 'Scrutiny', 'tab.graph': 'Graph', 'tab.history': 'History', 'tab.records': 'Records',

  // canvas
  'stage.aria': 'Flat pattern', 'parts.aria': 'Parts', 'tool.aria': 'Tool', 'tool.select': 'Select tool', 'tool.select.tip': 'Select · inspect curves', 'tool.measure': 'Measure tool', 'tool.measure.tip': 'Measure  M',
  'view.aria': 'View', 'zoom.out': 'Zoom out', 'zoom.out.tip': 'Zoom out  −', 'zoom.in': 'Zoom in', 'zoom.in.tip': 'Zoom in  +', 'zoom.fit': 'Fit pattern to view', 'zoom.fit.tip': 'Fit  F',
  'disp.aria': 'Display', 'disp.grid': 'Grid', 'disp.dims': 'Extent dimensions', 'disp.dims.tip': 'Extents', 'disp.labels': 'Curve names and seam marks', 'disp.labels.tip': 'Names',
  'disp.prev': 'Overlay the pattern of job #{id}', 'disp.prev.tip': 'Overlay job #{id} (dashed)',
  'wm.sup': 'Superseded — do not use', 'wm.rej': 'Rejected — do not use', 'wm.review': 'Not released · review required',
  'ro.idle': 'Hover a curve for its length and vertices. Click to select.', 'ro.measure1': 'Measure: click the first point. Points snap to computed vertices within 10 px.', 'ro.measure2': 'Measure: click the second point',
  'ro.dist': 'Distance {d} mm · ΔX {dx} · ΔY {dy}', 'ro.dist.live': 'Distance {d} mm · ΔX {dx} · ΔY {dy} (click to set)',
  'ro.hover': '{name} · {kind} · {layer} · length {len} mm', 'ro.hover.v': '{name} · {kind} · {layer} · length {len} mm · vertex {i}/{n} ({x}, {y})',
  'nopat.t': 'No pattern — input refused', 'nopat.default': 'The engine refused these values, so there is no pattern. Change the values and run it again; the refused request stays in the job list as NO RESULT.', 'nopat.go': 'Adjust the values in a new calculation',
  'cn.aria': 'Drawing notes', 'cn.side': 'Drawn as seen from the **{side}** of the formed part · provisional convention, not reviewed', 'cn.side.none': 'Marking side not stated by the engine',
  'cn.not': 'Not modelled: thickness, allowances, weld gap, kerf',
  'cn.seams': 'Seam edges {names}, hatched', 'cn.seams.tip': 'Named as seams by the engine. The DXF file does not mark them, nor the rolling axis.',
  'pv.role': 'pattern viewer',
  'pv.aria': 'Flat pattern of {part}: {n} curves, extents {w} by {h} millimetres. Arrow keys pan, plus and minus zoom, F fits, M measures. The curve table lists every curve.',
  'pv.first': 'first', 'pv.last': 'last', 'pv.v1': 'vertex 1', 'pv.free': 'free point', 'pv.on': '{curve} vertex {i}',

  // selection, measurement, curves, quantities
  'sel.none': 'Nothing selected. Click a curve in the pattern, or a name in the curve table.',
  'sel.curve': 'Curve', 'sel.entity': 'Entity', 'sel.layer': 'Layer', 'sel.length': 'Length', 'sel.vertices': 'Vertices', 'sel.first': 'First vertex (x, y)', 'sel.last': 'Last vertex (x, y)', 'sel.extent': 'Extent x × y',
  'sel.hint': 'Length is the value computed by the engine (hover for full precision); coordinates are the vertices sent to the DXF writer.',
  'sel.arc': 'Drawn here as {n} chords, at most {d} mm from the exact arc. The DXF carries the exact arc.',
  'meas.h': 'Measurement', 'meas.dist': 'Distance', 'meas.dxy': 'ΔX · ΔY', 'meas.from': 'From', 'meas.to': 'To', 'meas.hint': 'Straight-line distance in the development plane.',
  'cv.curve': 'Curve', 'cv.entity': 'Entity', 'cv.length': 'Length, mm', 'cv.select': 'Select curve {name} in the viewer', 'cv.outer': 'outer profile', 'cv.inner': 'opening',
  'tb.part': 'Part', 'tb.job': 'Job', 'tb.engine': 'Engine', 'tb.side': 'Marking side', 'tb.side.none': 'not stated by the engine', 'tb.status': 'Status', 'tb.nr': 'NOT RELEASED', 'cv.part': '{part}',
  'q.part': 'Part', 'q.class': 'Development class', 'q.class.note': '(proposed naming)', 'q.mside': 'Marking side', 'q.mside.note': 'provisional convention, not reviewed',
  'q.mside.v': 'drawn as seen from the **{side}** of the formed part', 'q.mside.none': 'not stated',
  'q.extent': 'Plate extent x × y', 'q.extent.note': 'from the vertices',
  'q.opening': 'Opening centre from plate centreline, along y', 'q.opening.note': 'from the vertices; its sign follows the sign of δ',
  'q.epsgov': '**Numerical error `eps_num` = {v} mm.** At this branch angle (near 180°) the rounding of the angle itself dominates the numerical error, and `eps_num` makes up at least half of the tolerance of the 3D check. eps_num is an a-priori rounding bound, not interval-certified. Check that it is acceptable for this part.',
  'q.epsnum': 'Numerical error', 'q.eps.note': 'a-priori bound, not interval-certified', 'q.epsrep': 'Curve representation error', 'q.sha': 'DXF SHA-256',
  'q.epsrep.cert': 'certified upper bound of the chord error of the drawn polyline, eps_num included (TOLERANCE_POLICY section 8.1)',
  'q.epsrep.est': 'NOT certified: for an eccentric reducer its quadrature term is an estimate, not a bound (TOLERANCE_POLICY section 8.2)',
  'q.epsrep.uncert': 'NOT certified: no bound could be proved for this pattern (TOLERANCE_POLICY section 8.2)',
  'q.epsrep.none': 'not applicable: eps_rep is 0 for this pattern (no polyline representation error)',
  'q.epsrep.unstated': 'the engine states no certification status for this pattern',
  'q.hint': 'Three decimals shown; hover a value for the full precision the engine returned. eps_num is an a-priori bound, not interval-certified; eps_rep is a certified bound except for the eccentric reducer.',
  'q.nounit': 'unit not stated', 'q.nounit.tip': 'The engine does not state a unit for this quantity',
  'dxf.withheld': 'DXF withheld', 'dxf.btn': 'DXF · not released', 'dxf.aria': 'Download DXF of {part}, not released', 'dxf.tip': 'Download DXF · {part} — file name carries NOT-RELEASED',
  'dxf.all': 'All {n} parts', 'dxf.all.aria': 'Download the DXF of all {n} parts, not released', 'dxf.all.tip': 'One file per part, each named NOT-RELEASED',
  'dxf.all.toast': '{n} DXF files requested, one per part. None is released.',

  // results: gates
  'g.n.pass': '{n} pass', 'g.n.inc': '{n} inconclusive', 'g.n.fail': '{n} fail', 'g.n.ni': '{n} not implemented', 'g.n.nr': '{n} not run',
  'g.filter.aria': 'Show gates', 'g.all': 'All {n}', 'g.expand': 'Expand all', 'g.collapse': 'Collapse all',
  'g.noresult': 'The engine refused this input, so only the gates that can be evaluated without a pattern are listed. Their results do not make this job reviewable.',
  'g.foot': 'A job is released only when every critical gate passes. G16 (engineer approval) has no implementation, so no job can be released by this build.',
  'g.more': ['Full text · {n} statement', 'Full text · {n} statements'], 'g.name': 'Engine name', 'g.points': ['{n} open point: {topics}', '{n} open points: {topics}'],
  'g.topic.mside': 'Marking side', 'g.topic.sref': 'Surface the radii refer to', 'g.topic.radii': 'Radii against the declared surface', 'g.topic.other': 'Further statements',

  // results: checks
  'c.n': '{a} of {b} pass', 'c.scope.aria': 'Show checks of', 'c.all': 'All parts', 'c.method': '{method} · independence class {cls}',
  'c.primary': 'Primary', 'c.reference': 'Reference', 'c.difference': 'Difference', 'c.tolerance': 'Tolerance',
  'c.foot': "Values in the engine's units (mm for lengths and distances); full precision on hover.", 'c.none.t': 'No verification checks', 'c.none.p': 'No pattern was produced, so there was nothing to verify.',

  // results: scrutiny
  's.known.h': 'Known-mistake check', 's.known.p': 'This job was compared with the lessons recorded in this project. A match never changes a value; it asks an engineer to look.',
  's.adv': 'Advisory', 's.adv.aria': 'Reason for advisory {id}', 's.adv.ph': 'Why is it right this time?', 's.adv.ack': 'Acknowledge with reason',
  's.obl.h': 'Scrutiny obligations from the correction', 's.obl.p': "Rule table: {rule}. Open obligations cannot be closed by time or budget — only by evidence or an engineer's recorded decision.",
  's.via': '{dir} · via {rel}', 's.obl.aria': 'Reason for obligation {id}', 's.obl.ph': 'What did you check?', 's.confirm': 'Confirm checked', 's.waive': 'Waive',
  's.resolved': '{note} — {by}, {when}',
  's.none.t': 'Nothing under scrutiny', 's.none.p': 'No advisory matched a recorded lesson, and no correction has raised obligations for this job.',
  's.e.adv': 'Enter why the value is right this time (recorded with your name).', 's.e.obl': 'Enter what you checked (the reason is recorded with your name).',
  's.t.adv': 'Advisory {id} acknowledged.', 's.t.obl': 'Obligation #{id} {decision}.',
  'dir.downstream': 'Downstream — derived from it', 'dir.lateral': 'Lateral — must still fit', 'dir.common_cause': 'Common cause — same convention',
  'dirs.downstream': 'downstream', 'dirs.lateral': 'lateral', 'dirs.common_cause': 'common cause',

  // results: graph, history, records
  'gr.open': 'Open as a drawn graph in AMOEBA', 'gr.none.t': 'No dependency edges', 'gr.none.p': 'The dependency graph has edges only for a job that produced a pattern. Open a job that has one.',
  'ev.none': 'Job #{id} recorded with no result', 'ev.computed': 'Job #{id} computed', 'ev.computed.d': '{fam}, entered by {by}. {n} records in the release closure.', 'ev.computed.corr': '{fam}, entered by {by}. {n} records in the release closure. Corrects {job}.',
  'ev.obl': '{n} scrutiny obligations raised', 'ev.obl.d': 'Created by following the dependency edges of the corrected job.', 'ev.obl.res': 'Obligation #{id} {state}', 'ev.adv': 'Advisory #{id} acknowledged',
  'ev.replay.d': 'Replay audit run by {by}: the closure was reproduced; {n} stored records checked, {f} failing.', 'ev.replay.d.bad': 'Replay audit run by {by}: the closure was NOT reproduced; {n} stored records checked, {f} failing.',
  'ev.sup': 'Superseded', 'ev.sup.d': 'Replaced by {job} after a correction. Files withheld.',
  'history.foot': 'Built from the timestamps stored with this job. Administrators see the full hash-chained audit log under Administration.',
  'r.closure': 'Release closure', 'r.n': 'Records in closure', 'r.wu': 'Declared work units', 'r.engine': 'Engine', 'r.replay': 'Replay audit',
  'r.checked': '{n} stored records checked, {f} failing.', 'r.replay.hint': 'A replay audit recomputes the job in a separate process and compares the closure hash.', 'r.h': 'Records ({n})',
  'replay.busy': 'Replaying…', 'replay.t.ok': 'Job #{id}: replay match — the closure was reproduced.', 'replay.t.bad': 'Job #{id}: REPLAY MISMATCH — the closure was not reproduced.',

  // results: changes after a correction
  'diff.entered': 'Entered values', 'diff.pattern': 'Pattern · {part}', 'diff.newpart': 'This part did not exist in the earlier job.', 'diff.ex': 'Extent along x', 'diff.ey': 'Extent along y',
  'diff.curve': 'Curve {name} length', 'diff.nochange': 'No computed value of this part changed.',
  'diff.lead': "Compared with {job}, which this job corrects. Only values that differ are listed; {n} compared values are unchanged. Angles in the engine's unit (rad).",
  'diff.value': 'Value', 'diff.change': 'Change',

  // correction dialog
  'corr.h': 'Record a correction to job #{id}',
  'corr.p': 'A correction never edits this job. It creates a new job, marks this one superseded, and raises an obligation for every record that depends on the corrected value, mates with something affected, or was entered under the same convention.',
  'corr.param': 'Parameter', 'corr.opt': '{label} — {key} (now {v})', 'corr.value': 'Corrected value', 'corr.reason': 'Reason (required, recorded with your name)', 'corr.reason.ph': 'e.g. drawing dimension is inside diameter',
  'corr.go': 'Record correction and propagate', 'corr.busy': 'Propagating…',
  'corr.e.num': 'Enter the corrected value using digits and a decimal point, for example 1.5. A comma is not accepted.', 'corr.e.same': 'That is the current value ({v}). A correction must change it.', 'corr.e.reason': 'Give the reason (at least 5 characters); it is recorded with your name.',
  'corr.toast': 'Correction recorded: {label} {a} → {b}. New job #{id}; {n} obligations raised, {open} need an engineer.',

  // manufacturing
  'mfg.sub': 'Every DXF file the engine has written, one row per part. None is released.',
  'mfg.note': '**No file here is released for cutting.** DXF file names carry the prefix NOT-RELEASED and the job state. The server withholds the files of rejected and superseded jobs.',
  'mfg.panel': 'DXF outputs', 'mfg.filter': 'Filter outputs', 'th.part': 'Part', 'th.extent': 'Plate extent x × y (mm)', 'th.curves': 'Curves', 'th.dxfsha': 'DXF SHA-256', 'th.jobstate': 'Job state',
  'mfg.opening': ['+ {n} opening', '+ {n} openings'], 'mfg.w.sup': 'withheld — superseded', 'mfg.w.sup.by': 'withheld — superseded by {job}', 'mfg.w.rej': 'withheld — rejected',
  'mfg.dl': 'DXF', 'mfg.dl.aria': 'Download DXF of job {id} part {part}, not released',
  'mfg.none.t': 'No DXF output yet', 'mfg.none.p': 'Each part of a job that produced a pattern gets one DXF file. Run a calculation to create one.',
  'mfg.foot': 'Extents are computed from the pattern vertices. Layers: CUT_OUTER (profile), CUT_INNER (openings). Every file carries the text NOT RELEASED, the unit and the marking side on the non-cutting layer NF_NOT_RELEASED.',
  'mfg.foot.refused': ['{n} job produced no pattern (input refused) and is not listed.', '{n} jobs produced no pattern (input refused) and are not listed.'],
  'mfg.planned.t': 'Manufacturing planning', 'mfg.planned.1': 'Plate division and stock fitting', 'mfg.planned.2': 'Seam and weld planning', 'mfg.planned.3': 'Allowance management (requires calibrated process data)',
  'mfg.planned.4': 'Manufacturing feasibility checks against machine limits', 'mfg.planned.5': 'Bend-line, marking and inner-profile layers; kerf handling',
  'mfg.planned.6': 'Seam edges, rolling axis and plate ends stated inside the DXF file; distinct layer colours', 'mfg.planned.7': 'CAM system acceptance testing',

  // verification
  'ver.sub': 'What stands between each job and release, and what still needs an engineer.',
  'ver.open': 'Open scrutiny obligations', 'th.affected': 'Affected', 'th.required': 'Required check', 'th.raised': 'Raised', 'ver.corrects': 'corrects #{id}',
  'ver.open.none.t': 'None open', 'ver.open.none.p': 'Every obligation raised so far has been discharged by evidence or by a recorded decision.',
  'ver.open.foot': '{n} obligations recorded in total. Open a job to confirm or waive one; every decision is recorded with its reason. The rule table that creates them is provisional and unreviewed. {ledger}', 'ver.ledger': 'Full ledger',
  'ver.status': 'Gate and check status per job', 'th.state.short': 'State', 'th.gates': 'Release gates', 'th.checks': 'Checks passed', 'th.oblopen': 'Open obligations', 'th.advopen': 'Open advisories',
  'ver.checks': '{a} of {b}', 'ver.checks.none': 'none run',
  'ver.status.foot': 'Superseded jobs are not listed ({n}). Gate G16, engineer approval, is not implemented, so every job shows at least one gate that cannot pass.',
  'ver.lessons': 'Lessons learned', 'ver.lessons.foot': 'Every later job in the same project is compared with these lessons.',
  'ver.planned.t': 'Quality workflow', 'ver.planned.1': 'Engineer approval — no job can reach APPROVED in this build', 'ver.planned.2': 'Findings by severity (critical / major / minor) and investigations',
  'ver.planned.3': 'Self-consistency audits A1–A4; replay on a separate machine or build', 'ver.planned.4': 'Obligations from drawing revisions and from verification failures (only parameter corrections raise them today)',

  // AMOEBA
  'am.sub': 'Engineering organism state — live contents of the append-only record store.', 'am.tabs.aria': 'AMOEBA views',
  'am.t.records': 'Records', 'am.t.activities': 'Activities', 'am.t.obligations': 'Obligations', 'am.t.lessons': 'Lessons', 'am.t.dependencies': 'Dependencies', 'am.t.mechanisms': 'Mechanisms',
  'am.rec.h': 'Records by family and kind', 'th.rfamily': 'Family', 'th.kind': 'Kind', 'th.count': 'Count', 'am.rec.none.t': 'No records yet', 'am.rec.none.p': 'Records appear when a project, document or job is created.',
  'am.rec.filter': 'Filter records', 'am.rec.foot': 'Records are content-addressed and append-only. Open a job to read individual records and their payloads.',
  'am.snap.h': 'Recent project snapshots', 'th.snapshot': 'Snapshot id', 'am.snap.none.t': 'No snapshots yet', 'am.snap.none.p': 'A snapshot is recorded when a project changes.',
  'am.act.h': 'Activities and declared logical work units', 'th.acttype': 'Activity type', 'th.distinct': 'Distinct activities', 'th.wu': 'Work units',
  'am.act.none.t': 'No activities yet', 'am.act.none.p': 'Activities are recorded when a job runs.',
  'am.act.foot': 'Identical activities on identical inputs are one record (content addressing), so repeated identical jobs do not add rows.',
  'am.obl.h': 'Scrutiny obligation ledger', 'th.direction': 'Direction', 'th.evidence': 'Evidence / decision', 'am.obl.none.t': 'No obligations recorded', 'am.obl.none.p': 'Obligations are raised when a correction propagates along dependency edges.',
  'am.obl.count': '{a} open of {b}', 'am.obl.filter': 'Filter obligations', 'am.obl.foot': 'Open obligations are resolved on the job, in the Scrutiny tab, with a recorded reason.',
  'am.les.h': 'Lessons and how often they matched', 'am.les.foot': 'A matching lesson raises an advisory on the new job (gate G20). Advisories are acknowledged on the job with a reason; a match never changes a value.',
  'am.dep.h': 'Dependency graph of a job', 'am.dep.opt': 'Job #{id} · {fam}', 'am.dep.opt.sup': 'Job #{id} · {fam} (superseded)', 'am.dep.rule': 'Rule table used for propagation: {rule}.',
  'am.dep.err': 'Could not load the graph', 'am.dep.none.p': 'Edges exist for jobs that produced a pattern.',
  'am.mech.impl': 'Implemented in this build', 'am.mech.not': 'Not implemented', 'am.mech.tag': 'planned',

  // not found, errors, loading
  'nf.title': 'Page not found', 'nf.crumb': 'Not found', 'nf.t': 'This page does not exist', 'nf.p': 'There is no page at {hash}. Use the navigation on the left, or go to the dashboard.',
  'err.title': 'Error', 'err.t': 'This page could not be loaded. Retry, or go back to the dashboard.', 'loading': 'Loading',

  // account
  'acc.sub': '{user} · role {role}', 'acc.pw.h': 'Change password', 'acc.pw.cur': 'Current password', 'acc.pw.new': 'New password (at least 10 characters)', 'acc.pw.rep': 'Repeat new password',
  'acc.pw.go': 'Change password', 'acc.pw.busy': 'Changing…', 'acc.pw.foot': 'Changing the password signs out every session of this account.',
  'acc.e.cur': 'Enter your current password.', 'acc.e.len': 'Use at least 10 characters.', 'acc.e.differ': 'The two new passwords differ.',
  'acc.pref.h': 'Preferences', 'acc.pref.theme': 'Theme', 'acc.pref.dark': 'Dark workstation (default)', 'acc.pref.light': 'Light', 'acc.pref.lang': 'Language',
  'acc.pref.rail': 'Navigation', 'acc.pref.rail.auto': 'Automatic (icons below 1440 px)', 'acc.pref.rail.open': 'Always show labels', 'acc.pref.rail.closed': 'Icons only',
  'acc.pref.reset': 'Reset panel sizes', 'acc.pref.reset.done': 'Panel sizes reset.', 'acc.pref.foot': 'Stored in this browser only, not on the server.',
  'acc.planned.t': 'Account security', 'acc.planned.1': 'Multi-factor authentication', 'acc.planned.2': 'Account recovery (an administrator must create a new account)', 'acc.planned.3': 'List and revoke individual sessions',

  // administration
  'adm.denied.t': 'Administrator role required', 'adm.denied.p': 'Users, security configuration and the audit log are visible to administrators only.', 'adm.denied.go': 'Back to the dashboard',
  'adm.sub': 'Users, the security configuration as running, and the hash-chained audit log.', 'adm.tabs.aria': 'Administration views',
  'adm.t.users': 'Users', 'adm.t.security': 'Security configuration', 'adm.t.audit': 'Audit log', 'adm.t.planned': 'Planned',
  'th.username': 'Username', 'th.role': 'Role', 'th.status': 'Status', 'th.actions': 'Actions', 'adm.you': '(you)', 'adm.active': 'active', 'adm.inactive': 'deactivated',
  'adm.deactivate': 'Deactivate', 'adm.reactivate': 'Reactivate', 'adm.confirm': 'Confirm: deactivate {user}', 'adm.users.foot': 'Project access is granted per project, on the project page.',
  'adm.add.h': 'Add user', 'adm.add.pass': 'Password (at least 10 characters)', 'adm.add.go': 'Add user', 'adm.add.busy': 'Adding…', 'adm.add.done': 'User {user} added as {role}.',
  'adm.e.user': 'Enter a username.', 'adm.e.pass': 'Use at least 10 characters.',
  'adm.off.done': '{user} deactivated; their sessions no longer work.', 'adm.on.done': '{user} reactivated.',
  'adm.sec.h': 'Security configuration (as running)', 'th.setting': 'Setting', 'th.value': 'Value', 'adm.sec.foot': 'Read from the running server. Settings are changed through environment variables at start-up, not from this page.',
  'adm.audit.filter': 'Filter audit log', 'th.when': 'When', 'th.user': 'User', 'th.action': 'Action', 'th.detail': 'Detail', 'adm.verify': 'Verify chain', 'adm.verify.busy': 'Verifying…',
  'adm.chain': 'Chain check: {state}', 'adm.chain.ok': 'INTACT', 'adm.chain.bad': 'BROKEN at entry {n}', 'adm.chain.over': 'over {n} entries.',
  'adm.chain.t.ok': 'Audit chain intact over {n} entries.', 'adm.chain.t.bad': 'Audit chain BROKEN at entry {n}.',
  'adm.planned.1': 'Multi-factor authentication, single sign-on', 'adm.planned.2': 'Built-in TLS, database encryption at rest, key rotation', 'adm.planned.3': 'Backup and restore tooling, retention policies',
  'adm.planned.4': 'Engineering standards and factory rule management', 'adm.planned.5': 'Separate tenants (isolation is per project within one installation)',
};

// ---------------------------------------------------------------- 日本語
// Wording rules used throughout: です・ます体 for sentences, noun phrases for labels and buttons; full-width
// punctuation （）：、。 inside Japanese text, half-width for identifiers, numbers and units; no space between
// Japanese and Latin text; a half-width space between a number and its unit (20 m, 50 MB).
// Safety notices use the plain prohibitions 「使用禁止」 and 「〜してはなりません」.
const JA = {
  'build.short': '開発版・未検証',
  'build.long': ' — 出力の製造使用禁止',
  'build.full': '開発版・未検証 — 出力を製造に使用してはなりません',
  'build.status': '開発版 · 未検証 · 製造使用禁止',
  'build.login': '**開発版です。** 検証されていません。いかなる出力も製造に使用してはなりません。',

  'state.APPROVED': '承認済み', 'state.REJECTED': '却下', 'state.SUPERSEDED': '旧版', 'state.NO_RESULT': '結果なし',
  'state.REVIEW_REQUIRED': '要レビュー', 'state.REFUSED': '入力拒否',
  'result.PASS': '合格', 'result.FAIL': '不合格', 'result.INCONCLUSIVE': '未確定', 'result.NOT_IMPLEMENTED': '未実装', 'result.NOT_RUN': '未実行',
  'obl.open': '未解決', 'obl.discharged': '解消済み', 'obl.discharged_by_decision': '判断により解消', 'obl.waived': '免除',
  'adv.open': '未確認', 'adv.acknowledged': '確認済み',
  'dec.confirmed': '確認済み', 'dec.waived': '免除',
  'role.admin': '管理者', 'role.engineer': '技術者', 'role.viewer': '閲覧者',
  'planned.tag': '計画中', 'planned.sub': 'このビルドでは未実装', 'planned.aria': '計画中：{title}',
  'yes': 'はい', 'no': 'いいえ', 'retry': '再試行', 'cancel': 'キャンセル', 'close': '閉じる', 'creating': '作成中…', 'storing': '保存中…',
  'job.n': 'ジョブ #{id}', 'job.n.lc': 'ジョブ #{id}', 'project.n': 'プロジェクト {id}', 'project.tip': 'プロジェクト #{id}',
  'unit.deg': '度',

  'eng.tag': 'エンジン出力・英語', 'srv.tag': 'サーバー出力・英語',
  'engsrc.aria': 'エンジン出力の表示：日本語訳または原文', 'engsrc.label': 'エンジン出力', 'engsrc.ja': '日本語訳', 'engsrc.en': '原文',
  'engsrc.tip': 'エンジンとサーバーが出力した文：確認済みの日本語訳、または英語の原文',

  'shell.skip': '本文へ移動', 'shell.nav.main': 'メインナビゲーション', 'shell.crumbs': '現在位置',
  'shell.rail.toggle': 'ナビゲーションのラベル表示を切り替え', 'shell.menu': 'ナビゲーションメニュー', 'shell.rail.tip': 'ナビゲーション  [', 'shell.brand.aria': 'NEXUS FABRIC、ダッシュボードへ',
  'shell.lang.aria': '言語', 'shell.lang.tip': '言語  L',
  'shell.bell': '通知', 'shell.bell.new': '通知（新着{n}件）',
  'shell.help.aria': 'キーボードショートカットとヘルプ', 'shell.help.tip': 'ショートカット  ?',
  'shell.theme.aria': 'ダークテーマとライトテーマを切り替え', 'shell.theme.tip': 'テーマ  T',
  'shell.account.aria': 'アカウント：{user}（権限：{role}）', 'shell.signout': 'ログアウト', 'shell.progress': '通信中',
  'shell.conn.ok': 'サーバー接続中', 'shell.conn.off': 'サーバーに接続できません', 'shell.engine': 'エンジン {id}',
  'shell.obl': '未解決の要精査項目 {n}件', 'shell.obl.aria': '未解決の要精査項目 {n}件', 'shell.shortcuts': 'ショートカット',
  'notes.title': '通知', 'notes.close': '通知を閉じる', 'notes.empty.t': 'まだありません', 'notes.empty.p': 'このセッションで行った操作の結果が、ここに表示されます。',
  'notes.foot': 'このブラウザのセッション内だけの記録です。サーバー側の通知サービスとジョブキューは計画中で、未実装です（ジョブは同期実行されます）。',
  'notes.k.err': 'エラー', 'notes.k.ok': '完了', 'notes.k.warn': '注意', 'notes.k.info': '情報', 'toast.dismiss': '通知を閉じる',

  'nav.g.overview': '概要', 'nav.g.engineering': '設計', 'nav.g.assurance': '保証', 'nav.g.system': 'システム',
  'nav.dashboard': 'ダッシュボード', 'nav.projects': 'プロジェクトと文書', 'nav.workspace': '設計ワークスペース', 'nav.manufacturing': '製造',
  'nav.amoeba': 'AMOEBA管理センター', 'nav.verification': '検証・品質', 'nav.admin': 'システム管理', 'nav.account': 'アカウント',

  'help.title': 'キーボードショートカット', 'help.list': 'ショートカット一覧', 'help.goto': '画面の移動', 'help.anywhere': '共通', 'help.viewer': '展開図ビューア', 'help.panels': 'パネル',
  'help.k.overlay': 'この一覧を表示', 'help.k.new': '新規計算', 'help.k.filter': '表示中の一覧の絞り込み欄へ移動', 'help.k.rail': 'ナビゲーションのラベルを表示／非表示',
  'help.k.theme': 'ダーク／ライトテーマ', 'help.k.lang': '英語／日本語', 'help.k.esc': '一覧を閉じる、入力欄から抜ける',
  'help.v.fit': '展開図を全体表示（ダブルクリックでも可）', 'help.v.zoom': '拡大・縮小（ホイールはカーソル位置を中心に拡大縮小）', 'help.v.pan': 'ビューアにフォーカスがあるとき表示を移動（ドラッグでも移動）',
  'help.v.measure': '2点間を計測（計算済みの頂点にスナップ）', 'help.v.part': '組立の部材を切り替え', 'help.v.esc': '計測を解除、次に選択を解除',
  'help.p.tabs': 'タブ列で：前／次のタブ', 'help.p.split': 'パネルの境界で：パネル幅を変更', 'help.p.curves': '曲線表の行で、ビューア上の曲線を選択',
  'help.foot': '{build}。アイコンボタンにポインタを合わせるかフォーカスすると、名称とショートカットが表示されます。',

  'login.doc': 'ログイン', 'login.h': 'ログイン', 'login.h.setup': '管理者アカウントの作成',
  'login.sub': '配管継手と板部材のための展開図。結果は製造用にリリースされていません。', 'login.sub.setup': 'このインストールの初回起動です。このアカウントでユーザーとプロジェクトを管理します。',
  'login.user': 'ユーザー名', 'login.pass': 'パスワード', 'login.pass.setup': 'パスワード（10文字以上）', 'login.token': 'セットアップトークン',
  'login.token.hint': 'このサーバーをインストールした人に表示されます（データディレクトリ内のファイル `setup-token`、または`NF_SETUP_TOKEN`の値）。使用できるのは1回限りです。',
  'login.go': 'ログイン', 'login.go.setup': '管理者を作成', 'login.busy': 'ログイン中…', 'login.busy.setup': '作成中…',
  'login.legal': 'すべてこのサーバー上で動作します。外部ネットワークのリソースは読み込みません。',
  'login.desktop.h': 'デスクトップモード：データの保存について',
  'login.desktop.p1': 'このプログラムは、データベースもデータフォルダー全体も暗号化しません。保存されたファイルには個別の暗号化がありますが、それによってこのコンピューター上のデータ全体が暗号化されるわけではありません。',
  'login.desktop.p2': 'データがどの程度保護されるかは、このコンピューターとその運用環境によって決まります。推奨：ディスクの暗号化、利用者ごとの個別アカウント、データフォルダーへのアクセスをこのプログラムの利用者だけに限定すること、保護されたバックアップ。',
  'login.desktop.p3': 'ディスクの暗号化は紛失・盗難にあったコンピューターを保護しますが、ディスクのロックが解除されている間に取られたデータフォルダーのコピーは保護しません。コンピューターのアカウントだけでは、データベースは暗号化されません。',
  'login.e.user': 'ユーザー名を入力してください。', 'login.e.pass': 'パスワードを入力してください。', 'login.e.len': '10文字以上にしてください。', 'login.pw.more': 'あと{n}文字必要です。', 'login.pw.ok': '10文字の最小条件を満たしています。複数の単語からなる長いパスフレーズほど強固です。', 'login.e.token': 'このインストールのセットアップトークンを入力してください。',
  'login.bad': 'ユーザー名またはパスワードが正しくありません。', 'login.ended': 'セッションが終了しました。もう一度ログインしてください。', 'login.changed': 'パスワードを変更しました。すべてのセッションからログアウトしました。もう一度ログインしてください。',
  'api.signin': 'ログインしてください。', 'api.unreachable': 'サーバーに接続できません。サーバーが動作していることを確認してから、再試行してください。', 'api.failed': 'リクエストが失敗しました（{status}）',
  'start.unreachable': 'サーバーに接続できません',

  'th.job': 'ジョブ', 'th.project': 'プロジェクト', 'th.family': '形状種別', 'th.parts': '部材', 'th.state': 'リリース状態', 'th.replay': '再実行監査', 'th.entered': '入力',
  'th.what': '誤りの内容', 'th.fromjob': '発生元ジョブ', 'th.matched': 'その後の一致回数', 'th.recorded': '記録',
  'th.record': '記録', 'th.relation': '関係', 'th.dependson': '依存先', 'th.name': '名称', 'th.docs': '文書', 'th.jobs': 'ジョブ', 'th.created': '作成',
  'jobs.none.t': 'ジョブはまだありません', 'jobs.none.cta': 'プロジェクトを作成', 'jobs.none.p': 'プロジェクトから計算を始めてください。部材の寸法を入力すると、エンジンが展開図を計算し、検算を行い、部材ごとにDXFファイルを1件出力します。',
  'new.calc': '新規計算', 'jobs.filter': 'ジョブを絞り込み', 'jobs.all': 'すべてのジョブ', 'to.dashboard': 'ダッシュボード',
  'parts.none': 'なし — 入力拒否', 'obl.open.n': '未解決 {n}件',
  'replay.match': '再実行一致', 'replay.mismatch': '再実行不一致', 'replay.none': '未実施',
  'lesson.corrected': '{fam} · {param}：{wrong} は誤り、正しくは {right}', 'lesson.other': '{kind} · {fam}',
  'lessons.none.t': '教訓はまだ記録されていません',
  'lessons.none.p': '教訓は、技術者が値を訂正したとき、検証で結果が却下されたとき、保存済みの結果が完全性チェックに失敗したときに、自動的に記録されます。',
  'flt.count': '{b}件中 {a}件', 'flt.nomatch': '「{q}」に一致するものはありません。', 'flt.clear': '絞り込みを解除',

  'dg.level0': '入力値・宣言', 'dg.level': '導出 · レベル{n}',
  'dg.idle': '記録を選択すると、その記録の依存先{amber}と、変更の影響が及ぶ範囲{blue}をたどれます。', 'dg.amber': '（黄色）', 'dg.blue': '（青色）',
  'dg.reach': '{node} は{n}件の記録に依存しています。これを変更すると{m}件に影響します：{list}。',
  'dg.reach.none': '{node} は{n}件の記録に依存しています。これを変更しても、影響を受ける下流の記録はありません。',
  'dg.layers.aria': '依存関係のレイヤー（記録{n}件、エッジ{e}本）', 'dg.node.aria': '{label}：依存先{a}件、参照元{b}件',
  'dg.layers.note': 'レイヤーは上から下へ読みます。まず入力値、次にそこから導出されたものです。',
  'dg.graph.aria': '依存関係グラフ（記録{n}件、エッジ{e}本）。スクロールできます。', 'dg.flow.note': '左から右へ読みます。入力値、次にそこから導出されたものです。',
  'dg.edges': 'エッジ一覧（{n}）',

  'dash.sub': 'エンジン {engine} · 稼働 {min}分', 'dash.totals': '集計',
  'dash.s.projects': 'プロジェクト', 'dash.s.jobs': 'ジョブ', 'dash.s.review': '要レビュー', 'dash.s.obl': '未解決の要精査項目',
  'dash.s.sup': '旧版', 'dash.s.none': '結果なし（入力拒否）', 'dash.s.rej': '却下', 'dash.s.records': '保存された記録',
  'dash.recent': '最近のジョブ', 'dash.needs': '技術者の判断が必要', 'dash.needs.open': 'ジョブ #{id} を開く', 'dash.needs.raised': '{when} に発生',
  'dash.needs.none.t': '未解決の要精査項目はありません', 'dash.needs.none.p': '要精査項目は、訂正の影響が他の記録に及ぶときに発生します。',
  'dash.state': '全ジョブの状態', 'dash.bar.aria': '要レビュー{review}、旧版{sup}、結果なし{none}、却下{rej}、承認済み{ok}',
  'dash.l.review': '要レビュー {n}', 'dash.l.sup': '旧版 {n}', 'dash.l.none': '結果なし {n} · 却下 {m}', 'dash.l.ok': '承認済み {n}',
  'dash.state.foot': 'このビルドでは、どのジョブも「承認済み」にはなりません。技術者承認ゲート（`G16`）が未実装のためです。「結果なし」は、エンジンが入力を受け付けず、何も計算しなかったことを示します。',
  'dash.partial': '件数は、全{total}件のうち最新の{n}件（サーバーが一覧に返す範囲）を対象としています。',
  'dash.planned.t': '処理キューとサーバー通知',
  'dash.planned.1': 'ジョブはリクエスト内で同期実行されます。監視するキューはありません。', 'dash.planned.2': '通知サービスはありません。ベルの一覧に出るのは、このブラウザのセッション内の出来事だけです。',

  'proj.crumb': 'プロジェクト', 'proj.sub': 'プロジェクトと、その元資料、およびそれぞれに記録されたスナップショットのハッシュ。', 'proj.panel': 'プロジェクト', 'proj.filter': 'プロジェクトを絞り込み',
  'proj.none.t': 'プロジェクトはまだありません', 'proj.none.viewer': '管理者が、まだあなたをプロジェクトに追加していません。', 'proj.none.p': '下のフォームから最初のプロジェクトを作成してください。ジョブと文書はプロジェクトに属します。',
  'proj.new': '新規プロジェクト', 'proj.f.name': '名称', 'proj.f.desc': '説明', 'proj.create': 'プロジェクトを作成', 'proj.e.name': 'プロジェクトの名称を入力してください。', 'proj.created': 'プロジェクト #{id} を作成しました。',
  'proj.planned.t': '改訂と比較', 'proj.planned.1': '図面の改訂履歴と、改訂間の差分', 'proj.planned.2': '2つのプロジェクトスナップショットの比較',
  'project.meta': 'プロジェクト #{id} · 作成者 {by} · {when}', 'project.jobs': 'ジョブ', 'project.docs': '元資料', 'project.access': 'アクセス権',
  'th.file': 'ファイル', 'th.dtype': '判定した形式', 'th.size': 'サイズ（バイト）', 'th.sha': 'SHA-256', 'th.uploaded': 'アップロード',
  'doc.mismatch': '拡張子と内容が不一致', 'docs.none.t': '保存された文書はありません', 'docs.none.p': '保存したファイルはハッシュ化し、暗号化します。証拠として保管するだけで、内容は読み取りません。',
  'doc.file.aria': '保存するファイル', 'doc.choose': 'ファイルを選択…', 'doc.nofile': 'ファイル未選択', 'doc.store': 'ファイルを保存', 'doc.e.nofile': '先にファイルを選択してください。',
  'doc.accepted': '内容で判定して受け付ける形式：PDF、PNG、JPEG、TIFF、DXF、STEP（最大50 MB）。ファイルはハッシュ化し、暗号化して保存します。**保存時に図面の内容は読み取りません。**PDFはDXFに変換するときだけ、隔離された別のプログラムが読み取ります。',
  'doc.snapshot': '現在のプロジェクトスナップショット：{id}', 'doc.snapshot.none': 'なし', 'doc.stored': '保存しました（{type}）。{interp}',
  'conv.view_side': '表示面', 'conv.plate_axes': '板の座標軸', 'conv.rolling_axis': '巻き軸', 'conv.seam_edges': '合わせ目の縁', 'conv.end_edges': '端の縁', 'conv.angle_datum': '角度の基準', 'conv.origin': '板の原点', 'conv.kerf': 'カーフ（切りしろ）', 'conv.status': '状態', 'conv.offset_delta': 'オフセット', 'conv.offset_sign': 'オフセットの符号', 'conv.branch_angle': '枝の傾き',
  'conv.action': 'DXFに変換', 'conv.action.aria': '{file} をDXFに変換', 'conv.label': 'PDFから変換・未検証・未リリース', 'conv.panel': 'PDFからDXFへの変換', 'conv.th.doc': '元資料', 'conv.th.page': 'ページ', 'conv.th.result': '結果', 'conv.title': 'PDFをDXFに変換', 'conv.rules': '変換プログラムは1ページのベクトル線を読み取ります。**縮尺の指定が必要です**：PDFには信頼できる実寸の縮尺がないため、推定しません。スキャンまたはラスターの図面は拒否します。寸法文字は読み取りません。結果は**PDFから変換・未検証・未リリース**です。板展開ではなく、計算に添付されることもありません。', 'conv.form': 'ページと縮尺', 'conv.f.page': 'ページ', 'conv.f.kind': '縮尺の指定方法', 'conv.f.kind.ratio': '図面の縮尺 1:N', 'conv.f.kind.mmpt': 'PDFの1ポイントあたりのミリメートル', 'conv.f.scale': '縮尺の値（必須）', 'conv.f.scale.hint': '縮尺1:10なら10を入力します。PDFの1ポイントあたりのミリメートルでは、1:1は25.4/72 = 0.352778です。', 'conv.f.tol': '曲線の弦公差（mm、任意）', 'conv.f.tol.hint': '空欄の場合は変換プログラムの既定値を使います。', 'conv.f.fill': '塗りつぶしの輪郭を含める', 'conv.f.text': '文字を含める', 'conv.submit': 'DXFに変換', 'conv.running': '変換中…', 'conv.done': '変換 #{id} を保存しました：PDFから変換・未検証・未リリース。', 'conv.refused': '変換プログラムが文書を拒否しました：{code}（上限：{limit}）。何も変換していません。', 'conv.refused.rec': 'この拒否は変換 #{id} として記録されています。', 'conv.e.scale': '縮尺を入力してください。PDFには信頼できる実寸の縮尺がないため、推定しません。', 'conv.e.nodoc': 'この文書はプロジェクトにありません。', 'conv.e.notpdf': 'PDFと判定された文書だけを変換できます。', 'conv.e.viewer': 'あなたの役割では変換を閲覧できますが、開始はできません。', 'conv.view.title': '変換 #{id}', 'conv.view.sub': '元資料 #{doc} から', 'conv.crumb.project': 'プロジェクト #{id}', 'conv.notflat': 'この図面はPDFの線を指定の縮尺で再現したものです。板展開ではなく、どの要件にも照らして確認されておらず、どの計算にも添付されていません。', 'conv.r.title': '変換レポート', 'conv.r.status': '結果', 'conv.r.code': 'コード', 'conv.r.message': 'メッセージ', 'conv.r.page': 'ページ', 'conv.r.scale': '縮尺', 'conv.r.tol': '弦公差', 'conv.r.bbox': '外接矩形', 'conv.r.source': '元資料のSHA-256', 'conv.r.version': '変換プログラムの版', 'conv.r.by': '変換者', 'conv.r.counts': '件数', 'conv.r.warnings': '警告', 'conv.r.skipped': '読み飛ばした文字（理由別）', 'conv.dl.dxf': 'DXFをダウンロード', 'conv.dl.report': 'レポートをダウンロード', 'conv.preview': '変換したファイルのプレビュー', 'conv.preview.note': 'DXFの線のみ表示し、文字は描画しません。板展開ではありません。', 'conv.preview.none': 'プレビューするものがありません。', 'conv.preview.aria': '変換したDXFの線画、ポリライン {n} 本', 'conv.scale.ratio': '1:{n}', 'conv.scale.mmpt': 'PDFの1ポイントあたり {x} mm', 'conv.st.refused': '拒否', 'conv.st.limit': '上限超過',
  'member.remove': '削除', 'member.remove.aria': '{user} をプロジェクトから削除', 'member.confirm': '確認：{user} を削除', 'members.none': 'メンバーはいません。',
  'members.hint': '管理者はすべてのプロジェクトを参照できます。その他のユーザーが参照できるのは、メンバーになっているプロジェクトだけです。',
  'member.add.label': 'ユーザーを追加', 'member.all': '有効なユーザーは全員メンバーです', 'member.add': 'メンバーを追加', 'member.opt': '{user} — {role}',
  'member.added': '{user} をプロジェクトに追加しました。', 'member.removed': '{user} をプロジェクトから削除しました。',

  'ws.sub': 'ジョブを開くと、展開図・検証・ゲート・来歴を確認できます。新しい計算も実行できます。', 'ws.all': 'すべてのジョブ',

  'math.noproj.t': '先にプロジェクトを作成してください', 'math.noproj.p': '計算はプロジェクトに記録されます。先にプロジェクトを作成し、このフォームに戻ってください。', 'math.noproj.go': 'プロジェクトへ',
  'math.viewer.t': 'この権限では、結果の閲覧はできますが、計算は実行できません', 'math.viewer.p': '管理者に「技術者」の権限を依頼してください。', 'math.viewer.go': 'ジョブを見る',
  'math.meta': '独立した検算付きの板展開。パラメータは手入力です。',
  'math.banner': '**暫定モデル：** 半径は、入力値をそのまま基準面の半径として使用します。板厚、各種の代（しろ）、溶接ギャップ、カーフは考慮していません。',
  'math.inputs.aria': '入力', 'math.pane.h': '入力', 'math.sec.job': 'ジョブ', 'math.f.project': 'プロジェクト', 'math.f.family': '形状種別', 'math.sec.params': 'パラメータ',
  'math.tag.from': 'ジョブ #{id} から複写', 'math.tag.example': '入力例',
  'math.hint.from': '以前のジョブから複写した値です。すべての値を確認してください。', 'math.hint.example': 'あらかじめ入っている数値は入力例であり、推奨値ではありません。',
  'math.run': '展開・検証・DXF出力', 'math.run.unusual': 'この値のまま実行', 'math.busy': '計算・検証中…',
  'math.sref.label': '入力した半径は、管壁のどの面の値ですか',
  'sref.unknown': '不明 — 要確定', 'sref.outside': '外面', 'sref.inside': '内面', 'sref.neutral': '中立面',
  'math.psref.same': '上の基準面と同じ', 'math.psref.hint': '部材ごとに、取合い半径がどの壁面を基準とするかを指定します。「上の基準面と同じ」のままなら、その部材には上の値を従来どおり用います。',
  'math.joint.none': '未宣言（単一の取合い線）', 'math.joint.set': 'セットオン（母管外面・枝管内面）',
  'math.joint.hint': '取合いがセットオンでなければ「未宣言」のままにしてください。未宣言では、入力した半径による取合い線を1本計算し、セットオンとは呼びません。セットオンには、母管の外面と枝管の内面の組合せが必要です。それ以外の宣言は拒否されます。',
  'joint.h': '取合い（母管と枝管）', 'joint.none.v': '未宣言', 'req.refused': '拒否',
  'v.joint.pair': 'セットオンの取合いには、母管の外面と枝管の内面の組合せが必要です。現在は、母管：{hs}、枝管：{bs}です。',
  'math.split.l': '入力パネルの幅を変更', 'math.split.r': '注記パネルの幅を変更', 'math.stage.h': 'パラメータ概略図', 'math.stage.tag': 'フォームの値から作図 · 計算結果ではありません',
  'math.limits.aria': 'モデル化の範囲', 'math.limits.h': 'このビルドの範囲', 'math.result.h': '得られる結果',
  'math.result.1': '部材ごとの展開図（曲線、長さ、数値誤差の推定値）。', 'math.result.2': '独立した検算による検証項目（計算値と参照値）。',
  'math.result.3': 'リリースゲート。技術者承認が未実装のため、すべてのジョブは**要レビュー**または**却下**のままです。', 'math.result.4': 'ファイル名に `NOT-RELEASED` が付くDXFファイル。',
  'math.result.5': 'エンジンが値を受け付けない場合、ジョブは理由とともに**結果なし**として記録され、何も計算されません。',
  'math.planned.h': '計画中', 'math.planned.tag': '未実装',
  'math.planned.1': '入力した寸法からの拘束求解', 'math.planned.2': '母管1本＋枝管1本以外の組立、複数の枝管、フランジやサポート',
  'math.planned.3': '角丸などの変形管、同径および部分的な枝管の相貫、斜円筒、円筒に近い円錐',
  'math.planned.4': '板厚・中立面の扱い、校正済みの工程データに基づく曲げ・ロール加工の伸び代', 'math.planned.5': '不確かさと公差の解析、区間演算で保証された誤差限界',
  'math.planned.6': '板割りと継目位置の最適化',
  'math.conv.h': '符号と投影の取り決め', 'math.conv.tag': '暫定',
  'math.conv.lead': 'このビルドの取り決め（仕様書02 §4.4）をクライアントが表示しています。計算結果のデータには含まれていません。暫定であり、製作技術者によるレビューは受けていません。',
  'math.conv.1': 'βが90°未満のとき、枝管は位置の値が大きい側（+x）へ傾きます。',
  'math.conv.2': '軸オフセットδが正のとき、枝管の軸は、側面図を見る人から遠ざかる向きにずれます（端面図では右へ）。',
  'math.conv.3': '端面図は、母管の位置の値が大きい側の端から、位置0の側を見た図です（第三角法の配置）。',
  'math.conv.4': 'δの符号を誤ると、組立全体が鏡像になりますが、検証項目はすべて合格のままです。',
  'math.notsent': '**送信していません。** {n}件の値は計算できません。印の付いた欄を確認してください。',
  'obl.onsup': '旧版の結果に対するもの',
  'obl.onsup.note': '旧版の結果に対する未解決の要精査項目が{n}件あります。一覧には含めていますが、未解決の作業には数えていません。',
  'math.plaus.t': '通常と異なる寸法です。単位（mm）を確認してください。', 'math.plaus.p': 'この値のまま実行するには、もう一度ボタンを押してください。',
  'math.toast.none': 'ジョブ #{id} は「結果なし」として記録されました。エンジンが入力を受け付けませんでした：{why}', 'math.toast.ok': 'ジョブ #{id} を計算し、検算を実施しました。状態：{state}。',
  'math.why.default': '展開図は生成されませんでした', 'math.state.default': 'ゲートを参照',
  'v.number': '{label}：数字と小数点で入力してください（例：1.5）。カンマは使えません。', 'v.below': '{label} = {v} mm は1e-6 mm未満です。このビルドで検証した最小寸法であり、工学上の規則ではなく実装上の下限です。', 'v.positive': '{label}は0より大きい値にしてください。', 'v.big': '{label} = {v} mm は20 mを超えています。', 'v.small': '{label} = {v} mm は1 mm未満です。',
  'v.r1r2': '大径側半径は、小径側半径より大きくしてください（大径側を先に入力）。', 'v.e': '小径側の偏心量は0以上にしてください。',
  'v.beta': 'このビルドでは、両軸のなす角は10°から170°の範囲にしてください。',
  'v.tangent': '｜軸オフセット｜＋枝管半径（{sum}）は、母管半径（{R}）より小さくしてください。接する場合、部分的な相貫、同径の相貫は未実装です。',
  'v.station': '枝管位置は、母管の長さの範囲内にしてください。',
  'v.outside': '取合い半径を「外面」と宣言した場合、{d}（{dv}）を{c}（{cv}）より大きくすることはできません。板厚が負になります。',
  'v.inside': '取合い半径を「内面」と宣言した場合、{d}（{dv}）を{c}（{cv}）より小さくすることはできません。板厚が負になります。',
  'v.neutral': '取合い半径を「中立面」と宣言した場合、{d}（{dv}）は{c}（{cv}）と等しくしてください。',

  'sk.msg': '概略図を表示するには、すべての欄に正の数値を入力してください。',
  'sk.cap': 'フォームの値から原寸比で作図しています（単位mm）。パラメータの意味を確かめるための補助図であり、計算された展開図ではなく、検証もされていません。',
  'sk.notdrawn': '図示していないもの：{items}。', 'sk.dev': '{label} {v} mm → 展開幅 2π·r = {w} mm', 'sk.aria': '入力した{fam}の寸法の概略図：{dims}',
  'sk.d.length': '長さ', 'sk.d.hlen': '母管長さ', 'sk.d.station': '位置',
  'sk.v.sidev': '側面図', 'sk.v.endv': '端面図', 'sk.v.cone': '側面図（大径側が下）',
  'sk.v.side': '側面図', 'sk.v.side.part': '側面図（母管は一部のみ表示）', 'sk.v.end': '端面図（位置の値が大きい側の端から見る）', 'sk.v.end.part': '端面図（母管軸方向から見る）',
  'sk.ax.away': '+y、+δ：奥へ', 'sk.ax.toward': '+x：手前へ', 'sk.ax.station': 'x、位置', 'sk.ax.x': 'x',
  'sk.delta.pos': 'δ {v}：枝管の軸は、側面図を見る人から遠ざかる向きにずれています', 'sk.delta.neg': 'δ {v}：枝管の軸は、側面図を見る人に近づく向きにずれています', 'sk.delta.zero': 'δ 0：2本の軸は交わります',

  'calc.notfound.title': 'ジョブが見つかりません', 'calc.notfound.crumb': '該当なし', 'calc.notfound.t': 'ジョブ #{id} は存在しません',
  'calc.notfound.p': '作成されていないか、あなたがメンバーではないプロジェクトのジョブです。',
  'job.corrects': '#{id} を訂正', 'job.enteredby': '入力者 {by}',
  'job.successor': '現行のジョブ #{id} を開く', 'job.correct': '訂正を記録…', 'job.replay': '再実行監査を実行', 'job.replay.tip': '別プロセスで再計算し、閉包ハッシュを比較',
  'job.clone': 'この値で新規計算', 'job.clone.tip': 'この値を初期値にして新規計算',
  'ban.sup': '**訂正により {job} に置き換えられた旧版です。** この結果を使用してはなりません。ファイルは提供されません。記録としてのみ保管されています。',
  'ban.none': '**結果なし：** エンジンがこの入力を受け付けませんでした：{why}。何も計算されていません。レビュー、訂正、製造の対象はありません。',
  'ban.rej': '**却下：** 重要ゲートが不合格です。このジョブのいかなる出力も使用してはなりません。',
  'ban.review': '**要レビュー：** この結果を製造用にリリースすることはできません。理由はすべて「ゲート」タブに表示されています。', 'ban.review.sum': '未合格のゲート：{list}（全{total}件中{n}件）。各理由は「ゲート」タブにあります。', 'ban.none.min': '閾値：`s_end`はこの形状で{n} mmより大きい必要があります（エンジンが算出）。',
  'ban.none.default': '展開図は生成されませんでした',
  'pane.props.aria': '入力値とプロパティ', 'pane.props.h': 'プロパティ', 'pane.results.aria': '結果・検証・ゲート', 'split.props': 'プロパティパネルの幅を変更', 'split.results': '結果パネルの幅を変更',
  'req.h': '入力値', 'req.tag': '手入力', 'req.correct': '訂正', 'req.correct.aria': '{label}を訂正', 'req.hint': '元図面とは連携していません。',
  'sel.h': '選択', 'curves.h': '曲線', 'quant.h': '展開図の諸元', 'assume.h': '前提条件',
  'tabs.aria': '結果', 'tab.changes': '変更点', 'tab.gates': 'ゲート', 'tab.checks': '検証項目', 'tab.scrutiny': '精査', 'tab.graph': '依存関係', 'tab.history': '履歴', 'tab.records': '記録',

  'stage.aria': '展開図', 'parts.aria': '部材', 'tool.aria': 'ツール', 'tool.select': '選択ツール', 'tool.select.tip': '選択 · 曲線を調べる', 'tool.measure': '計測ツール', 'tool.measure.tip': '計測  M',
  'view.aria': '表示範囲', 'zoom.out': '縮小', 'zoom.out.tip': '縮小  −', 'zoom.in': '拡大', 'zoom.in.tip': '拡大  +', 'zoom.fit': '展開図を全体表示', 'zoom.fit.tip': '全体表示  F',
  'disp.aria': '表示項目', 'disp.grid': 'グリッド', 'disp.dims': '外形寸法', 'disp.dims.tip': '外形寸法', 'disp.labels': '曲線名と継目の印', 'disp.labels.tip': '曲線名',
  'disp.prev': 'ジョブ #{id} の展開図を重ねて表示', 'disp.prev.tip': 'ジョブ #{id} を重ねる（破線）',
  'wm.sup': '旧版 — 使用禁止', 'wm.rej': '却下 — 使用禁止', 'wm.review': '未リリース · 要レビュー',
  'ro.idle': '曲線にポインタを合わせると、長さと頂点を表示します。クリックで選択。', 'ro.measure1': '計測：1点目をクリック。10 px以内にある計算済みの頂点にスナップします。', 'ro.measure2': '計測：2点目をクリック',
  'ro.dist': '距離 {d} mm · ΔX {dx} · ΔY {dy}', 'ro.dist.live': '距離 {d} mm · ΔX {dx} · ΔY {dy}（クリックで確定）',
  'ro.hover': '{name} · {kind} · {layer} · 長さ {len} mm', 'ro.hover.v': '{name} · {kind} · {layer} · 長さ {len} mm · 頂点 {i}/{n}（{x}, {y}）',
  'nopat.t': '展開図なし — 入力拒否', 'nopat.default': 'エンジンがこの値を受け付けなかったため、展開図はありません。値を変えて再実行してください。拒否された要求は、ジョブ一覧に「結果なし」として残ります。', 'nopat.go': '値を修正して新規計算',
  'cn.aria': '図面の注記', 'cn.side': '成形後の部材を**{side}**側から見た図 · 暫定規約・未レビュー', 'cn.side.none': 'けがき面はエンジンから示されていません',
  'cn.not': '未考慮：板厚、各種の代、溶接ギャップ、カーフ',
  'cn.seams': '継目の辺 {names}（斜線の印）', 'cn.seams.tip': 'エンジンが継目として名付けた辺です。DXFファイルには、継目もロールの軸も記されていません。',
  'pv.role': '展開図ビューア',
  'pv.aria': '{part}の展開図：曲線{n}本、外形 {w} × {h} mm。矢印キーで移動、＋と−で拡大縮小、Fで全体表示、Mで計測。すべての曲線は曲線表に一覧があります。',
  'pv.first': '始点', 'pv.last': '終点', 'pv.v1': '頂点1', 'pv.free': '任意の点', 'pv.on': '{curve} 頂点{i}',

  'sel.none': '未選択です。展開図の曲線、または曲線表の名前をクリックしてください。',
  'sel.curve': '曲線', 'sel.entity': '図形要素', 'sel.layer': 'レイヤー', 'sel.length': '長さ', 'sel.vertices': '頂点数', 'sel.first': '始点（x, y）', 'sel.last': '終点（x, y）', 'sel.extent': '範囲 x × y',
  'sel.hint': '長さはエンジンが計算した値です（ポインタを合わせると全桁を表示）。座標は、DXF出力に渡された頂点です。',
  'sel.arc': '画面上は{n}本の弦で描いています（真の円弧との差は最大 {d} mm）。DXFには真の円弧が出力されます。',
  'meas.h': '計測', 'meas.dist': '距離', 'meas.dxy': 'ΔX · ΔY', 'meas.from': '始点', 'meas.to': '終点', 'meas.hint': '展開平面上の直線距離です。',
  'cv.curve': '曲線', 'cv.entity': '要素', 'cv.length': '長さ, mm', 'cv.select': 'ビューアで曲線 {name} を選択', 'cv.outer': '外形', 'cv.inner': '開口',
  'tb.part': '部材', 'tb.job': 'ジョブ', 'tb.engine': 'エンジン', 'tb.side': 'けがき面', 'tb.side.none': 'エンジンからは示されていません', 'tb.status': '状態', 'tb.nr': '未リリース', 'cv.part': '{part}',
  'q.part': '部材', 'q.class': '展開クラス', 'q.class.note': '（提案段階の名称）', 'q.mside': 'けがき面', 'q.mside.note': '暫定規約・未レビュー',
  'q.mside.v': '成形後の部材を**{side}**側から見た図', 'q.mside.none': '記載なし',
  'q.extent': '板の外形寸法 x × y', 'q.extent.note': '頂点から算出',
  'q.opening': '開口中心の、板の中心線からのずれ（y方向）', 'q.opening.note': '頂点から算出。符号はδの符号に従います',
  'q.epsgov': '**数値誤差 `eps_num` = {v} mm。** この分岐角（180°に近い角度）では、角度そのものの丸めが数値誤差の大部分を占め、`eps_num`が3D検証の許容値の半分以上を占めています。これは丸め誤差の先験的な上界であり、区間演算による保証はありません。この部材で許容できるか確認してください。',
  'q.epsnum': '数値誤差', 'q.eps.note': '先験的な上界（区間演算による保証なし）', 'q.epsrep': '曲線の近似誤差', 'q.sha': 'DXF SHA-256',
  'q.epsrep.cert': '描かれた折れ線の弦の誤差について、認証された上界です（eps_numを含みます。許容誤差の方針文書 第8.1節）。',
  'q.epsrep.est': '認証されていません。偏心レデューサでは、その求積項は推定値であり、上界ではありません（許容誤差の方針文書 第8.2節）。',
  'q.epsrep.uncert': '認証されていません。このパターンでは上界を証明できませんでした（許容誤差の方針文書 第8.2節）。',
  'q.epsrep.none': '該当しません。このパターンではeps_repは0で、折れ線の近似誤差はありません。',
  'q.epsrep.unstated': 'このパターンの認証の状態は、エンジンから示されていません。',
  'q.hint': '小数第3位まで表示しています。値にポインタを合わせると、エンジンが返した全桁を表示します。eps_num は先験的な上界（区間演算による保証なし）です。eps_rep は偏心レデューサを除き、認証された上界です。',
  'q.nounit': '単位の記載なし', 'q.nounit.tip': 'エンジンは、この量の単位を示していません',
  'dxf.withheld': 'DXF提供停止', 'dxf.btn': 'DXF · 未リリース', 'dxf.aria': '{part}のDXFをダウンロード（未リリース）', 'dxf.tip': 'DXFをダウンロード · {part} — ファイル名にNOT-RELEASEDが付きます',
  'dxf.all': '全{n}部材', 'dxf.all.aria': '全{n}部材のDXFをダウンロード（未リリース）', 'dxf.all.tip': '部材ごとに1ファイル。すべてのファイル名にNOT-RELEASEDが付きます',
  'dxf.all.toast': 'DXFファイル{n}件を要求しました（部材ごとに1件）。いずれも未リリースです。',

  'g.n.pass': '合格 {n}', 'g.n.inc': '未確定 {n}', 'g.n.fail': '不合格 {n}', 'g.n.ni': '未実装 {n}', 'g.n.nr': '未実行 {n}',
  'g.filter.aria': '表示するゲート', 'g.all': 'すべて {n}', 'g.expand': 'すべて開く', 'g.collapse': 'すべて閉じる',
  'g.noresult': 'エンジンがこの入力を受け付けなかったため、展開図がなくても評価できるゲートだけを表示しています。その結果によって、このジョブがレビュー可能になるわけではありません。',
  'g.foot': 'ジョブがリリースされるのは、すべての重要ゲートに合格した場合だけです。`G16`（技術者承認）が未実装のため、このビルドでは、どのジョブもリリースできません。',
  'g.more': '全文 · 記述{n}件', 'g.name': 'エンジン上の名称', 'g.points': '未解決{n}点：{topics}',
  'g.topic.mside': 'けがき面', 'g.topic.sref': '半径の基準面', 'g.topic.radii': '半径と基準面の整合', 'g.topic.other': 'その他の記述',

  'c.n': '{b}件中 {a}件合格', 'c.scope.aria': '表示する検証項目', 'c.all': '全部材', 'c.method': '{method} · 独立性クラス {cls}',
  'c.primary': '計算値', 'c.reference': '参照値', 'c.difference': '差', 'c.tolerance': '許容差',
  'c.foot': '値はエンジンの単位です（長さと距離はmm）。ポインタを合わせると全桁を表示します。', 'c.none.t': '検証項目はありません', 'c.none.p': '展開図が生成されなかったため、検証する対象がありません。',

  's.known.h': '既知の誤りとの照合', 's.known.p': 'このジョブを、このプロジェクトに記録された教訓と照合しました。一致しても値は変更されません。技術者に確認を求めるだけです。',
  's.adv': '注意喚起', 's.adv.aria': '注意喚起{id}の理由', 's.adv.ph': '今回はこの値で正しい理由', 's.adv.ack': '理由を付けて確認',
  's.obl.h': '訂正により発生した要精査項目', 's.obl.p': '規則表：{rule}。未解決の要精査項目は、時間や予算の都合では解消できません。解消できるのは、証拠、または技術者の記録された判断だけです。',
  's.via': '{dir} · 関係 {rel}', 's.obl.aria': '要精査項目{id}の理由', 's.obl.ph': '確認した内容', 's.confirm': '確認済みとして記録', 's.waive': '免除',
  's.resolved': '{note} — {by}、{when}',
  's.none.t': '精査の対象はありません', 's.none.p': '記録された教訓に一致する注意喚起はなく、このジョブに対して訂正による要精査項目も発生していません。',
  's.e.adv': '今回はこの値で正しい理由を入力してください（あなたの名前とともに記録されます）。', 's.e.obl': '確認した内容を入力してください（理由は、あなたの名前とともに記録されます）。',
  's.t.adv': '注意喚起{id}を確認済みにしました。', 's.t.obl': '要精査項目 #{id}：{decision}。',
  'dir.downstream': '下流：これから導出されたもの', 'dir.lateral': '横方向：取合いの再確認が必要', 'dir.common_cause': '共通原因：同じ規約で入力',
  'dirs.downstream': '下流', 'dirs.lateral': '横方向', 'dirs.common_cause': '共通原因',

  'gr.open': 'AMOEBAでグラフとして表示', 'gr.none.t': '依存関係のエッジはありません', 'gr.none.p': '依存関係のエッジは、展開図が生成されたジョブにだけあります。展開図のあるジョブを開いてください。',
  'ev.none': 'ジョブ #{id} を「結果なし」として記録', 'ev.computed': 'ジョブ #{id} を計算', 'ev.computed.d': '{fam}、入力者 {by}。リリース閉包の記録 {n}件。', 'ev.computed.corr': '{fam}、入力者 {by}。リリース閉包の記録 {n}件。{job}を訂正したものです。',
  'ev.obl': '要精査項目{n}件が発生', 'ev.obl.d': '訂正されたジョブの依存関係エッジをたどって作成されました。', 'ev.obl.res': '要精査項目 #{id}：{state}', 'ev.adv': '注意喚起 #{id} を確認済みに',
  'ev.replay.d': '再実行監査（実行者 {by}）：閉包は再現されました。保存済みの記録{n}件を確認、不合格{f}件。', 'ev.replay.d.bad': '再実行監査（実行者 {by}）：閉包は再現されませんでした。保存済みの記録{n}件を確認、不合格{f}件。',
  'ev.sup': '旧版に', 'ev.sup.d': '訂正により{job}に置き換えられました。ファイルは提供されません。',
  'history.foot': 'このジョブとともに保存されたタイムスタンプから作成しています。管理者は「システム管理」で、ハッシュチェーン化された監査ログの全体を参照できます。',
  'r.closure': 'リリース閉包', 'r.n': '閉包内の記録数', 'r.wu': '宣言された作業単位', 'r.engine': 'エンジン', 'r.replay': '再実行監査',
  'r.checked': '保存済みの記録{n}件を確認、不合格{f}件。', 'r.replay.hint': '再実行監査では、別プロセスでジョブを再計算し、閉包ハッシュを比較します。', 'r.h': '記録（{n}）',
  'replay.busy': '再実行中…', 'replay.t.ok': 'ジョブ #{id}：再実行一致。閉包は再現されました。', 'replay.t.bad': 'ジョブ #{id}：再実行不一致。閉包は再現されませんでした。',

  'diff.entered': '入力値', 'diff.pattern': '展開図 · {part}', 'diff.newpart': 'この部材は、以前のジョブにはありませんでした。', 'diff.ex': 'x方向の範囲', 'diff.ey': 'y方向の範囲',
  'diff.curve': '曲線 {name} の長さ', 'diff.nochange': 'この部材の計算値に変更はありません。',
  'diff.lead': 'このジョブが訂正した{job}との比較です。差のある値だけを表示しています。比較した値のうち{n}件は変更なしです。角度はエンジンの単位（rad）です。',
  'diff.value': '項目', 'diff.change': '変化量',

  'corr.h': 'ジョブ #{id} の訂正を記録',
  'corr.p': '訂正によって、このジョブが書き換えられることはありません。新しいジョブを作成し、このジョブを旧版とします。さらに、訂正した値に依存する記録、影響を受けるものと取り合う記録、同じ規約で入力された記録のそれぞれに、要精査項目を発生させます。',
  'corr.param': 'パラメータ', 'corr.opt': '{label} — {key}（現在 {v}）', 'corr.value': '訂正後の値', 'corr.reason': '理由（必須。あなたの名前とともに記録されます）', 'corr.reason.ph': '例：図面寸法は内径だった',
  'corr.go': '訂正を記録して影響を伝播', 'corr.busy': '伝播中…',
  'corr.e.num': '訂正後の値を、数字と小数点で入力してください（例：1.5）。カンマは使えません。', 'corr.e.same': 'それは現在の値（{v}）です。訂正では値を変える必要があります。', 'corr.e.reason': '理由を入力してください（5文字以上）。あなたの名前とともに記録されます。',
  'corr.toast': '訂正を記録しました：{label} {a} → {b}。新しいジョブ #{id}。要精査項目{n}件が発生し、うち{open}件は技術者の判断が必要です。',

  'mfg.sub': 'エンジンが出力したすべてのDXFファイルを、部材ごとに1行で表示します。リリース済みのものはありません。',
  'mfg.note': '**ここにあるファイルは、いずれも切断用にリリースされていません。** DXFのファイル名には、接頭辞`NOT-RELEASED`とジョブの状態が付きます。却下されたジョブと旧版のジョブのファイルは、サーバーが提供しません。',
  'mfg.panel': 'DXF出力', 'mfg.filter': '出力を絞り込み', 'th.part': '部材', 'th.extent': '板の外形寸法 x × y（mm）', 'th.curves': '曲線数', 'th.dxfsha': 'DXF SHA-256', 'th.jobstate': 'ジョブの状態',
  'mfg.opening': '＋開口{n}', 'mfg.w.sup': '提供停止 — 旧版', 'mfg.w.sup.by': '提供停止 — {job}により旧版', 'mfg.w.rej': '提供停止 — 却下',
  'mfg.dl': 'DXF', 'mfg.dl.aria': 'ジョブ{id}・部材{part}のDXFをダウンロード（未リリース）',
  'mfg.none.t': 'DXF出力はまだありません', 'mfg.none.p': '展開図が生成されたジョブでは、部材ごとに1件のDXFファイルが出力されます。計算を実行すると作成されます。',
  'mfg.foot': '外形寸法は、展開図の頂点から算出しています。レイヤー：`CUT_OUTER`（外形）、`CUT_INNER`（開口）。すべてのファイルは、切断に使わないレイヤー`NF_NOT_RELEASED`に、`NOT RELEASED`の文字、単位、けがき面を記しています。',
  'mfg.foot.refused': '展開図が生成されなかったジョブ（入力拒否）{n}件は、一覧に含まれていません。',
  'mfg.planned.t': '製造計画', 'mfg.planned.1': '板割りと定尺材への割付', 'mfg.planned.2': '継目と溶接の計画', 'mfg.planned.3': '各種の代（しろ）の管理（校正済みの工程データが必要）',
  'mfg.planned.4': '機械の能力に照らした製造可否の確認', 'mfg.planned.5': '曲げ線・けがき・内形のレイヤー、カーフの扱い',
  'mfg.planned.6': '継目の辺・ロールの軸・板の端をDXFファイル内に明記、レイヤーごとの色分け', 'mfg.planned.7': '`CAM`システムでの受入試験',

  'ver.sub': '各ジョブのリリースまでに残っている事項と、技術者の判断が必要な事項。',
  'ver.open': '未解決の要精査項目', 'th.affected': '対象', 'th.required': '必要な確認', 'th.raised': '発生', 'ver.corrects': '#{id} を訂正',
  'ver.open.none.t': '未解決の項目はありません', 'ver.open.none.p': 'これまでに発生した要精査項目は、すべて証拠または記録された判断によって解消されています。',
  'ver.open.foot': '要精査項目は合計{n}件記録されています。確認または免除するには、ジョブを開いてください。すべての判断は、理由とともに記録されます。項目を発生させる規則表は暫定であり、未レビューです。{ledger}', 'ver.ledger': '台帳の全件',
  'ver.status': 'ジョブごとのゲートと検証項目の状況', 'th.state.short': '状態', 'th.gates': 'リリースゲート', 'th.checks': '合格した検証項目', 'th.oblopen': '未解決の要精査項目', 'th.advopen': '未確認の注意喚起',
  'ver.checks': '{a} / {b}', 'ver.checks.none': '未実行',
  'ver.status.foot': '旧版のジョブは表示していません（{n}件）。ゲート`G16`（技術者承認）が未実装のため、どのジョブにも、合格できないゲートが少なくとも1つあります。',
  'ver.lessons': '教訓', 'ver.lessons.foot': '同じプロジェクトの以降のジョブは、すべてこれらの教訓と照合されます。',
  'ver.planned.t': '品質ワークフロー', 'ver.planned.1': '技術者承認 — このビルドでは、どのジョブも「承認済み」になりません', 'ver.planned.2': '重大度別（重大／主要／軽微）の指摘事項と調査',
  'ver.planned.3': '自己整合性監査A1–A4、別のマシンまたはビルドでの再実行', 'ver.planned.4': '図面の改訂や検証の不合格による要精査項目（現在は、パラメータの訂正でのみ発生します）',

  'am.sub': '設計オーガニズムの状態 — 追記専用の記録ストアの現在の内容。', 'am.tabs.aria': 'AMOEBAの表示',
  'am.t.records': '記録', 'am.t.activities': 'アクティビティ', 'am.t.obligations': '要精査項目', 'am.t.lessons': '教訓', 'am.t.dependencies': '依存関係', 'am.t.mechanisms': '機構',
  'am.rec.h': '系統・種類別の記録', 'th.rfamily': '系統', 'th.kind': '種類', 'th.count': '件数', 'am.rec.none.t': '記録はまだありません', 'am.rec.none.p': 'プロジェクト、文書、またはジョブを作成すると、記録されます。',
  'am.rec.filter': '記録を絞り込み', 'am.rec.foot': '記録は内容アドレス方式で、追記専用です。個々の記録とその内容は、ジョブを開いて参照できます。',
  'am.snap.h': '最近のプロジェクトスナップショット', 'th.snapshot': 'スナップショットID', 'am.snap.none.t': 'スナップショットはまだありません', 'am.snap.none.p': 'プロジェクトが変更されると、スナップショットが記録されます。',
  'am.act.h': 'アクティビティと、宣言された論理作業単位', 'th.acttype': 'アクティビティの種類', 'th.distinct': '異なるアクティビティ数', 'th.wu': '作業単位',
  'am.act.none.t': 'アクティビティはまだありません', 'am.act.none.p': 'ジョブを実行すると、アクティビティが記録されます。',
  'am.act.foot': '同一の入力に対する同一のアクティビティは、1つの記録になります（内容アドレス方式）。そのため、同じジョブを繰り返しても行は増えません。',
  'am.obl.h': '要精査項目の台帳', 'th.direction': '方向', 'th.evidence': '証拠・判断', 'am.obl.none.t': '要精査項目は記録されていません', 'am.obl.none.p': '要精査項目は、訂正の影響が依存関係のエッジに沿って伝播するときに発生します。',
  'am.obl.count': '全{b}件中 未解決{a}件', 'am.obl.filter': '要精査項目を絞り込み', 'am.obl.foot': '未解決の要精査項目は、ジョブの「精査」タブで、理由を記録して解消します。',
  'am.les.h': '教訓と一致回数', 'am.les.foot': '教訓に一致すると、新しいジョブに注意喚起が発生します（ゲート`G20`）。注意喚起は、ジョブ上で理由を付けて確認します。一致しても値は変更されません。',
  'am.dep.h': 'ジョブの依存関係グラフ', 'am.dep.opt': 'ジョブ #{id} · {fam}', 'am.dep.opt.sup': 'ジョブ #{id} · {fam}（旧版）', 'am.dep.rule': '伝播に使用した規則表：{rule}。',
  'am.dep.err': 'グラフを読み込めませんでした', 'am.dep.none.p': 'エッジは、展開図が生成されたジョブに存在します。',
  'am.mech.impl': 'このビルドで実装済み', 'am.mech.not': '未実装', 'am.mech.tag': '計画中',

  'nf.title': 'ページが見つかりません', 'nf.crumb': '該当なし', 'nf.t': 'このページは存在しません', 'nf.p': '{hash} に該当する画面はありません。左のナビゲーションから選ぶか、ダッシュボードへ戻ってください。',
  'err.title': 'エラー', 'err.t': 'この画面を読み込めませんでした。再試行するか、ダッシュボードへ戻ってください。', 'loading': '読み込み中',

  'acc.sub': '{user} · 権限 {role}', 'acc.pw.h': 'パスワードの変更', 'acc.pw.cur': '現在のパスワード', 'acc.pw.new': '新しいパスワード（10文字以上）', 'acc.pw.rep': '新しいパスワード（確認）',
  'acc.pw.go': 'パスワードを変更', 'acc.pw.busy': '変更中…', 'acc.pw.foot': 'パスワードを変更すると、このアカウントのすべてのセッションからログアウトします。',
  'acc.e.cur': '現在のパスワードを入力してください。', 'acc.e.len': '10文字以上にしてください。', 'acc.e.differ': '新しいパスワードが一致しません。',
  'acc.pref.h': '表示設定', 'acc.pref.theme': 'テーマ', 'acc.pref.dark': 'ダーク（既定）', 'acc.pref.light': 'ライト', 'acc.pref.lang': '言語',
  'acc.pref.rail': 'ナビゲーション', 'acc.pref.rail.auto': '自動（1440 px未満ではアイコンのみ）', 'acc.pref.rail.open': '常にラベルを表示', 'acc.pref.rail.closed': 'アイコンのみ',
  'acc.pref.reset': 'パネル幅を初期化', 'acc.pref.reset.done': 'パネル幅を初期化しました。', 'acc.pref.foot': 'このブラウザにだけ保存され、サーバーには保存されません。',
  'acc.planned.t': 'アカウントのセキュリティ', 'acc.planned.1': '多要素認証', 'acc.planned.2': 'アカウントの復旧（現在は、管理者が新しいアカウントを作成する必要があります）', 'acc.planned.3': '個別セッションの一覧表示と無効化',

  'adm.denied.t': '管理者権限が必要です', 'adm.denied.p': 'ユーザー、セキュリティ設定、監査ログを参照できるのは、管理者だけです。', 'adm.denied.go': 'ダッシュボードへ戻る',
  'adm.sub': 'ユーザー、稼働中のセキュリティ設定、ハッシュチェーン化された監査ログ。', 'adm.tabs.aria': 'システム管理の表示',
  'adm.t.users': 'ユーザー', 'adm.t.security': 'セキュリティ設定', 'adm.t.audit': '監査ログ', 'adm.t.planned': '計画中',
  'th.username': 'ユーザー名', 'th.role': '権限', 'th.status': '状態', 'th.actions': '操作', 'adm.you': '（自分）', 'adm.active': '有効', 'adm.inactive': '無効',
  'adm.deactivate': '無効化', 'adm.reactivate': '再有効化', 'adm.confirm': '確認：{user} を無効化', 'adm.users.foot': 'プロジェクトへのアクセス権は、各プロジェクトのページで個別に付与します。',
  'adm.add.h': 'ユーザーを追加', 'adm.add.pass': 'パスワード（10文字以上）', 'adm.add.go': 'ユーザーを追加', 'adm.add.busy': '追加中…', 'adm.add.done': 'ユーザー {user} を{role}として追加しました。',
  'adm.e.user': 'ユーザー名を入力してください。', 'adm.e.pass': '10文字以上にしてください。',
  'adm.off.done': '{user} を無効化しました。このユーザーのセッションは使えなくなりました。', 'adm.on.done': '{user} を再有効化しました。',
  'adm.sec.h': 'セキュリティ設定（稼働中の値）', 'th.setting': '設定項目', 'th.value': '値', 'adm.sec.foot': '稼働中のサーバーから読み取った値です。設定は、起動時の環境変数で変更します。この画面からは変更できません。',
  'adm.audit.filter': '監査ログを絞り込み', 'th.when': '日時', 'th.user': 'ユーザー', 'th.action': '操作', 'th.detail': '詳細', 'adm.verify': 'チェーンを検証', 'adm.verify.busy': '検証中…',
  'adm.chain': 'チェーンの検証：{state}', 'adm.chain.ok': '改ざんなし', 'adm.chain.bad': 'エントリー{n}で破損', 'adm.chain.over': '（{n}件）',
  'adm.chain.t.ok': '監査チェーンに改ざんはありません（{n}件）。', 'adm.chain.t.bad': '監査チェーンが、エントリー{n}で破損しています。',
  'adm.planned.1': '多要素認証、シングルサインオン', 'adm.planned.2': '`TLS`の内蔵、データベースの保存時暗号化、鍵のローテーション', 'adm.planned.3': 'バックアップと復元のツール、保存期間のポリシー',
  'adm.planned.4': '技術基準と工場ルールの管理', 'adm.planned.5': 'テナントの分離（現在の分離は、1つのインストール内のプロジェクト単位です）',
};

const MSG = { en: EN, ja: JA };
// BH-014: one message per refusal code (docs/backend/REFUSAL_CODES.md). `{fields}`, `{limit}` and `{measured}` are the
// request fields the rule concerns and the limit and measured value the engine computes (only where it computes them).
// A code without a message here shows the server's own text.
const REFUSAL_EN = {
  'refusal.request_not_object': 'The request must be a `JSON` object.',
  'refusal.family_unknown': 'The family is missing, or it is not one of the five families.',
  'refusal.field_missing': 'A value is required for `{fields}`.',
  'refusal.field_wrong_type': '`{fields}` has a value of the wrong type.',
  'refusal.field_unknown': '`{fields}` is not a field of this family. Nothing is ignored silently: remove it.',
  'refusal.surface_reference_required': 'Give `surface_reference`, or give both `header_surface_reference` and `branch_surface_reference`.',
  'refusal.request_malformed': 'The request is not valid `JSON`, or it was not sent as `application/json`.',
  'refusal.value_not_finite': '`{fields}` must be a finite number.',
  'refusal.value_not_positive': '`{fields}` must be greater than zero.',
  'refusal.offset_negative': 'The offset `e` must be zero or more.',
  'refusal.magnitude_above_limit': '`{fields}` is {measured} mm, above {limit} mm, the largest size this build has been exercised with. This is an implementation limit, not an engineering rule.',
  'refusal.dimension_below_limit': '`{fields}` is {measured} mm, below {limit} mm, the smallest dimension this build has been exercised with. This is an implementation limit, not an engineering rule.',
  'refusal.reducer_radii_order': '`r1` must be greater than `r2`: the large end comes first.',
  'refusal.reducer_near_cylindrical': 'The radii are nearly equal (the sine of the half-angle is {measured}, below {limit}); this build does not implement the apex-free method for that case.',
  'refusal.developed_angle_overlaps': 'The developed angle is {measured} rad, so the sector would overlap itself (the limit is {limit} rad).',
  'refusal.branch_angle_out_of_range': 'The angle between the axes (`beta_deg`) must lie strictly between 0 and 180 degrees, and the developed branch must not be longer than 1e8 mm, the largest size this build is exercised with.',
  'refusal.branch_angle_not_verifiable': 'At this angle between the axes (`beta_deg`) the independent verification cannot confirm the pattern: its own rounding grows as `1/sin²(beta)` near 0 and 180 degrees and is no longer bounded inside its tolerance (bound {measured} mm, tolerance left {limit} mm), and its check fails. Nothing is offered. A limit of the verification in this build, not an engineering rule.',
  'refusal.branch_tangent_or_partial': 'The branch must be smaller than the header: `delta` plus `branch_radius` must stay below `header_radius` (measured {measured}, limit {limit}). Tangent and partial intersections, including the equal-diameter tee, are not built.',
  'refusal.square_end_too_short': 'The square end `s_end` is too short: it must reach past the saddle, the curve where the two pipes meet (the engine says: does not lie beyond the saddle): make it longer.',
  'refusal.opening_outside_shell': 'The branch opening does not lie inside the header length: check `branch_station` and `header_length`.',
  'refusal.chord_error_unreachable': 'This curve cannot be drawn within the chord error this build allows, with the most segments it uses.',
  'refusal.radius_dev_exceeds_outside': 'The development radius ({measured}) is larger than the contact radius ({limit}) for the outside surface, which would mean a negative wall thickness. Correct the radii or the declared surface.',
  'refusal.radius_dev_below_inside': 'The development radius ({measured}) is smaller than the contact radius ({limit}) for the inside surface, which would mean a negative wall thickness. Correct the radii or the declared surface.',
  'refusal.radius_neutral_mismatch': 'For the neutral surface the development radius must equal the contact radius ({limit}); it is {measured}. Correct the radii or the declared surface.',
  'refusal.surface_reference_invalid': 'The surface reference must be `outside`, `inside`, `neutral` or `unknown`.',
  'refusal.joint_pairing_invalid': 'A set-on joint needs the branch surface `inside` and the header surface `outside`.',
  'refusal.joint_type_unknown': 'The joint must be left out, or be `set_on`.',
  'refusal.bend_angle_out_of_range': 'The bend deduction is defined for angles above zero and up to 90 degrees.',
  'refusal.reason_required': 'Every correction needs a reason of at least five bytes (a Japanese character counts as three bytes).',
  'refusal.reason_too_long': 'The reason is too long: at most 2000 characters.',
  'refusal.correction_not_for_family': 'Corrections with propagation are made for header-with-branch jobs only.',
  'refusal.correction_unknown_parameter': 'That is not a numeric parameter of this job.',
  'refusal.correction_value_unchanged': 'The new value equals the current value, so there is nothing to correct.',
  'refusal.correction_parameter_not_correctable': 'This parameter cannot be corrected.',
  'refusal.correction_parameter_not_recorded': 'This job has no record of that parameter, so the correction cannot be propagated.',
  'refusal.correction_on_superseded_job': 'This result is already superseded: correct the current one.',
  'refusal.correction_on_refused_job': 'This request was refused and has no result to correct: submit a new calculation.',
  'refusal.correction_needs_recompute': 'This result cannot be corrected: it failed its integrity check or was computed by a superseded engine. Recompute it first.',
  'refusal.stored_result_unreadable': 'The stored result cannot be read.',
  'refusal.stored_request_unreadable': 'The stored request cannot be read.',
  'refusal.corrected_result_failed_verification': 'The corrected request produced a pattern that failed its own verification.',
  'refusal.no_pattern_produced': 'The request produced no pattern and stated no rule.',
};
const REFUSAL_JA = {
  'refusal.request_not_object': 'リクエストは`JSON`のオブジェクトでなければなりません。',
  'refusal.family_unknown': '`family`がないか、5つの形式のいずれでもありません。',
  'refusal.field_missing': '`{fields}`の値が必要です。',
  'refusal.field_wrong_type': '`{fields}`の値の型が正しくありません。',
  'refusal.field_unknown': '`{fields}`はこの形式の項目ではありません。黙って無視されることはないので、削除してください。',
  'refusal.surface_reference_required': '`surface_reference`、または`header_surface_reference`と`branch_surface_reference`の両方を入力してください。',
  'refusal.request_malformed': 'リクエストが正しい`JSON`ではないか、`application/json`で送られていません。',
  'refusal.value_not_finite': '`{fields}`は有限の数値でなければなりません。',
  'refusal.value_not_positive': '`{fields}`は0より大きい値でなければなりません。',
  'refusal.offset_negative': 'オフセット`e`は0以上でなければなりません。',
  'refusal.magnitude_above_limit': '`{fields}`は{measured}mmで、このビルドで検証した最大寸法{limit}mmを超えています。これは実装上の上限であり、工学上の規則ではありません。',
  'refusal.dimension_below_limit': '`{fields}`は{measured}mmで、このビルドで検証した最小寸法{limit}mmを下回っています。これは実装上の下限であり、工学上の規則ではありません。',
  'refusal.reducer_radii_order': '`r1`は`r2`より大きくなければなりません（大きい方の端を先に入力します）。',
  'refusal.reducer_near_cylindrical': '半径がほぼ等しく（半頂角の正弦は{measured}で、{limit}未満）、このビルドでは、その場合の頂点を使わない方法を実装していません。',
  'refusal.developed_angle_overlaps': '展開角は{measured}radで、扇形が自身と重なります（限界は{limit}rad）。',
  'refusal.branch_angle_out_of_range': '軸の間の角度（`beta_deg`）は0度より大きく180度より小さくなければなりません。また、展開した枝管の長さは、このビルドで扱った最大寸法の1e8 mmを超えてはなりません。',
  'refusal.branch_angle_not_verifiable': 'この軸間角度（`beta_deg`）では、独立した検証で型紙を確認できません。検証自身の丸め誤差は0度と180度の近くで`1/sin²(beta)`に比例して大きくなり、許容値の内側に収まる保証がなくなります（上界{measured} mm、残りの許容値{limit} mm）。さらに、その検証で不合格になりました。何も提供しません。これはこのビルドの検証上の限界であり、技術上の規則ではありません。',
  'refusal.branch_tangent_or_partial': '枝管は母管より小さくなければなりません。`delta`と`branch_radius`の和は`header_radius`を下回る必要があります（測定値{measured}、限界{limit}）。接する交差と部分的な交差（等径のティーを含む）は作図していません。',
  'refusal.square_end_too_short': '直切り端`s_end`が短すぎます：相貫線の先まで届いていません（相貫線＝2本の管が交わる曲線）。長くしてください。',
  'refusal.opening_outside_shell': '枝管の開口が母管の長さの内側に入っていません。`branch_station`と`header_length`を確認してください。',
  'refusal.chord_error_unreachable': 'このビルドが許容する弦の誤差の範囲内では、使用できる最大の分割数でも描けない曲線です。',
  'refusal.radius_dev_exceeds_outside': '展開半径（{measured}）が外面の接触半径（{limit}）より大きく、負の板厚になります。半径か基準面を修正してください。',
  'refusal.radius_dev_below_inside': '展開半径（{measured}）が内面の接触半径（{limit}）より小さく、負の板厚になります。半径か基準面を修正してください。',
  'refusal.radius_neutral_mismatch': '中立面では、展開半径は接触半径（{limit}）と等しくなければなりません。現在は{measured}です。半径か基準面を修正してください。',
  'refusal.surface_reference_invalid': '基準面は`outside`、`inside`、`neutral`、`unknown`のいずれかでなければなりません。',
  'refusal.joint_pairing_invalid': 'セットオンの取合いには、枝管の基準面`inside`と母管の基準面`outside`が必要です。',
  'refusal.joint_type_unknown': '取合いは省くか、`set_on`としてください。',
  'refusal.bend_angle_out_of_range': '曲げ控除は、0度より大きく90度以下の角度について定義されています。',
  'refusal.reason_required': '訂正には理由が必要です（5バイト以上。日本語の1文字は3バイトとして数えます）。',
  'refusal.reason_too_long': '理由が長すぎます（2000文字以内）。',
  'refusal.correction_not_for_family': '伝播を伴う訂正は、母管と枝管の組合せの計算だけで行えます。',
  'refusal.correction_unknown_parameter': 'この計算の数値パラメータではありません。',
  'refusal.correction_value_unchanged': '新しい値が現在の値と同じため、訂正はありません。',
  'refusal.correction_parameter_not_correctable': 'このパラメータは訂正できません。',
  'refusal.correction_parameter_not_recorded': 'この計算にはそのパラメータの記録がないため、訂正を伝播できません。',
  'refusal.correction_on_superseded_job': 'この結果はすでに差し替えられています。現在の結果を訂正してください。',
  'refusal.correction_on_refused_job': 'この依頼は拒否されており、訂正する結果がありません。新しい計算を送信してください。',
  'refusal.correction_needs_recompute': 'この結果は訂正できません（整合性の検査に失敗したか、差し替え済みのエンジンで計算されています）。先に再計算してください。',
  'refusal.stored_result_unreadable': '保存された結果を読めません。',
  'refusal.stored_request_unreadable': '保存された依頼を読めません。',
  'refusal.corrected_result_failed_verification': '訂正後の依頼から得た展開図が、自身の検証に失敗しました。',
  'refusal.no_pattern_produced': 'この依頼は展開図を生成せず、規則も示されませんでした。',
};
Object.assign(EN, REFUSAL_EN);
Object.assign(JA, REFUSAL_JA);
export const CATALOGUE = MSG;

// ---------------------------------------------------------------- identifiers the engine and the server send
// [English label, Japanese label]. English `null` = the client shows the identifier in words, as it always did.
// The identifier itself stays on screen in monospace next to a Japanese label: it is what the DXF, the
// records and the API call the thing, and what a reviewer needs in order to check the label.
const ID = {
  family: {
    cylinder_shell: ['Cylindrical shell', '円筒胴'],
    cone_frustum: ['Concentric reducer', '同心レジューサ'],
    eccentric_reducer: ['Eccentric reducer', '偏心レジューサ'],
    header_with_branch: ['Header with branch (assembly)', '母管＋枝管（組立）'],
    branch_stub: ['Branch stub', '枝管スタブ'],
    header_shell_with_opening: [null, '開口付き母管胴'],
  },
  familyLong: {
    cylinder_shell: ['Cylindrical shell', '円筒胴'],
    cone_frustum: ['Concentric reducer (right cone frustum)', '同心レジューサ（直円錐台）'],
    eccentric_reducer: ['Eccentric reducer (oblique cone frustum)', '偏心レジューサ（斜円錐台）'],
    header_with_branch: ['Assembly: header shell with branch (two parts, one joint)', '組立：母管胴と枝管（2部材・取合い1箇所）'],
    branch_stub: ['Branch stub on cylindrical header', '円筒母管に取り付く枝管スタブ'],
  },
  param: {
    r_dev: ['Development radius', '展開半径'], length: ['Length', '長さ'],
    r1: ['Large-end radius', '大径側半径'], r2: ['Small-end radius', '小径側半径'], h: ['Axial height', '軸方向高さ'], e: ['Offset of small end', '小径側の偏心量'],
    header_radius: ['Header contact radius', '母管の取合い半径'], header_dev_radius: ['Header development radius', '母管の展開半径'], header_length: ['Header shell length', '母管胴の長さ'],
    branch_station: ['Branch position along header', '母管軸方向の枝管位置'], branch_radius: ['Branch contact radius', '枝管の取合い半径'], branch_dev_radius: ['Branch development radius', '枝管の展開半径'],
    beta_deg: ['Angle between axes', '両軸のなす角'], delta: ['Axis offset', '軸オフセット'],
    s_end: ['Branch square end, distance along branch axis', '枝管の直切り端（枝管軸に沿った距離）'],
    'branch_stub.s_end': ['Square end, distance along branch axis', '直切り端（枝管軸に沿った距離）'],
    surface_reference: [null, '半径の基準面'],
    header_surface_reference: ['Header contact radius: wall surface', '母管の取合い半径の基準面'],
    branch_surface_reference: ['Branch contact radius: wall surface', '枝管の取合い半径の基準面'],
    joint: ['Joint type', '取合いの形式'],
  },
  // third entry: the unit the engine uses for the quantity (it sends none)
  quantity: {
    seam_gap: ['Seam gap (flat pattern, small end)', '継目の隙間（展開図・小径側）', 'mm'], seam_gap_resolution: ['Seam gap resolution', '継目の隙間の分解能', 'mm'],
    area: ['Area', '面積', 'mm²'], gross_area: ['Gross area, before the opening', '総面積（開口前）', 'mm²'], net_area: ['Net area, after the opening', '正味面積（開口を差し引いた後）', 'mm²'], opening_area: ['Opening area', '開口の面積', 'mm²'], developed_width: ['Developed width', '展開幅', 'mm'], length: ['Length', '長さ', 'mm'], opening_perimeter: ['Opening perimeter', '開口の周長', 'mm'],
    opening_x_min: ['Opening, smallest x', '開口のx最小値', 'mm'], opening_x_max: ['Opening, largest x', '開口のx最大値', 'mm'],
    developed_angle: ['Developed angle', '展開角', 'rad'], sector_angle: ['Sector angle', '扇形の中心角', 'rad'],
    rho_at_0: ['Slant radius at 0°', '頂点からの母線長（0°）', 'mm'], rho_at_180: ['Slant radius at 180°', '頂点からの母線長（180°）', 'mm'],
    s_at_0: ['Cut distance at 0°', '切断距離（0°）', 'mm'], s_at_90: ['Cut distance at 90°', '切断距離（90°）', 'mm'], s_max: ['Longest cut distance', '切断距離の最大値', 'mm'], s_min: ['Shortest cut distance', '切断距離の最小値', 'mm'],
    apex_radius_large: ['Apex radius, large end', '頂点からの半径（大径側）', 'mm'], apex_radius_small: ['Apex radius, small end', '頂点からの半径（小径側）', 'mm'],
    chord_large_arc: ['Chord of large arc', '大径側円弧の弦長', 'mm'], slant_length: ['Slant length', '母線長', 'mm'], sin_half_angle: ['Sine of half apex angle', '半頂角の正弦', '–'],
  },
  part: { shell: [null, '胴板'], reducer: [null, 'レジューサ'], branch: [null, '枝管'], header: [null, '母管'], interface: [null, '取合い'] },
  // what each named curve is, from the engine source: the names alone are the only record of seams and ends
  curve: {
    seam_a: ['seam edge A', '継目の辺A'], seam_b: ['seam edge B', '継目の辺B'],
    large_end: ['large-end edge', '大径側の端'], small_end: ['small-end edge', '小径側の端'],
    saddle_cut: ['saddle cut against the header', '母管との相貫切断線'], square_end: ['square end', '直切り端'],
    bottom: ['end edge at axial position 0', '管端の辺（軸方向0側）'], top: ['end edge at the far end', '管端の辺（軸方向の終端側）'],
    branch_opening: ['opening for the branch', '枝管用の開口'],
  },
  entity: { line: [null, '直線'], arc: [null, '円弧'], polyline: [null, 'ポリライン'] },
  layer: { CUT_OUTER: ['profile', '外形'], CUT_INNER: ['opening', '開口'] },
  devclass: { 'E-C': ['exact development, closed form', '厳密な展開・閉形式'], 'E-N': ['exact development, evaluated numerically', '厳密な展開・数値評価'] },
  side: { outside: ['outside', '外面'], inside: ['inside', '内面'] },
  sref: { unknown: [null, '不明'], outside: [null, '外面'], inside: [null, '内面'], neutral: [null, '中立面'] },
  joint: { set_on: ['Set-on', 'セットオン'] },
  gate: {
    G00: [null, '現行版であること'], G01: [null, '来歴の完全性'], G02: [null, '未解決の曖昧さがないこと'], G05: [null, '展開クラスの割当て'], G06: [null, '前提条件の裏付け'],
    G08: [null, '数値誤差の限界'], G10: [null, '検証'], G11: [null, '輪郭の位相'], G14: [null, 'アルゴリズムの評価状況'], G15: [null, '出力の完全性'], G16: [null, '技術者承認'],
    G18: [null, '要精査項目の解消'], G19: [null, '自己整合性'], G20: [null, '既知の誤りとの照合'], G21: [null, 'エンジンが現行版であること'], G22: [null, '保存済み結果の完全性'],
  },
  check: {
    bottom_edge_equals_circumference: [null, '端の辺の長さ＝円周長'], seam_edges_equal_length: [null, '両方の継目の辺が同じ長さ'], seam_length_equals_input: [null, '継目の辺の長さ＝入力した長さ'],
    area: [null, '面積'], large_arc_equals_circumference: [null, '大径側の円弧長＝円周長'], small_arc_equals_circumference: [null, '小径側の円弧長＝円周長'],
    seam_equals_slant_length: [null, '継目の辺の長さ＝母線長'], fold_back_radius_residual: [null, '立体に戻したときの半径の残差'], large_end_present: [null, '大径側の端があること'],
    apex_free_development_agrees: [null, '頂点を使わない展開との一致'], large_edge_equals_circumference: [null, '大径側の辺の長さ＝円周長'], small_edge_equals_circumference: [null, '小径側の辺の長さ＝円周長'],
    seam_equals_3d_ruling: [null, '継目の辺の長さ＝3Dの母線長'], cut_points_on_header_surface: [null, '切断線の点が母管面上にあること'], cut_points_on_branch_surface: [null, '切断線の点が枝管面上にあること'],
    saddle_handedness_matches_declared_side: [null, '相貫切断線の向きが、宣言したけがき面と一致'], outline_counter_clockwise: [null, '外形が反時計回り'],
    square_end_equals_circumference: [null, '直切り端の長さ＝円周長'], outline_extent_along_header_axis: [null, '母管軸方向の外形寸法'], outline_extent_around_header: [null, '母管周方向の外形寸法'],
    opening_points_on_branch_surface: [null, '開口の点が枝管面上にあること'], shell_edge_equals_circumference: [null, '胴板の辺の長さ＝円周長'],
    opening_closed_simple_inside_shell: [null, '開口が閉じた単純な輪郭で、胴板の内側にあること'], parts_declare_the_same_marking_side: [null, '両部材のけがき面の宣言が同じ'],
    branch_cut_lies_on_header_opening: [null, '枝管の切断線が母管の開口線上にあること'], header_opening_lies_on_branch_cut: [null, '母管の開口線が枝管の切断線上にあること'],
    opening_on_intersection_of_branch_geometry: [null, '開口が、枝管の形状の交線上にあること'], saddle_on_intersection_of_header_geometry: [null, '相貫切断線が、母管の形状の交線上にあること'],
    // pending native fabrication-engineer review (C7)
    opening_centre_on_the_declared_delta_side: [null, '開口の中心が、宣言したデルタ側にあること'],
    opening_matches_3d_intersection: [null, '開口が、3Dの交線と一致すること'],
    opening_developed_length_equals_3d_arc_length: [null, '開口の展開長＝3Dの円弧長'],
    opening_area_equals_surface_area: [null, '開口の面積＝表面積'],
    header_circle_3d_arc_length: [null, '母管の円の3D円弧長'],
    saddle_matches_3d_intersection: [null, '相貫切断線が、3Dの交線と一致すること'],
    saddle_on_the_declared_delta_side: [null, '相貫切断線が、宣言したデルタ側にあること'],
    saddle_developed_length_equals_3d_arc_length: [null, '相貫切断線の展開長＝3Dの円弧長'],
    pattern_area_equals_surface_area: [null, '展開図の面積＝表面積'],
    square_end_3d_arc_length: [null, '直切り端の3D円弧長'],
  },
  rfamily: { source: [null, '原資料'], claim: [null, '主張'], relation: [null, '関係'], activity: [null, 'アクティビティ'], finding: [null, '所見'], assessment: [null, '評価'], attestation: [null, '証明'] },
  rkind: {
    dxf_quality_report: [null, 'DXF品質ゲートの報告'],
    'build.develop': [null, '展開計算'], 'build.dxf': [null, 'DXF出力'], 'build.intersect': [null, '相貫線の計算'], 'verify.family': [null, '形状種別ごとの検証'], 'verify.interface': [null, '取合いの検証'],
    'verify.profile': [null, '輪郭の検査'], 'verify.reimport': [null, '再読込と照合'], gate_evaluation: [null, 'ゲート評価'], interface_evidence: [null, '取合いの検証結果'], profile_report: [null, '輪郭検査の報告'],
    reimport_report: [null, '再読込・照合の報告'], replay_audit: [null, '再実行監査'], verification_evidence: [null, '検証結果'], correction: [null, '訂正'], obligation_resolution: [null, '要精査項目の解消'],
    advisory_acknowledgement: [null, '注意喚起の確認'], convention: [null, '規約'], development_request: [null, '展開計算の依頼'], dxf_output: [null, 'DXF出力'], interface: [null, '取合い'],
    intersection_curve: [null, '相貫線'], parameter: [null, 'パラメータ'], part: [null, '部材'], pattern: [null, '展開図'], engineering_outcome: [null, 'エンジンの判定'], lesson: [null, '教訓'],
    obligation: [null, '要精査項目'], advisory: [null, '注意喚起'], snapshot: [null, 'スナップショット'], source_artifact: [null, '原資料ファイル'],
  },
  rel: {
    input: [null, '入力'], developed_from: [null, '展開の元'], verifies: [null, '検証の対象'], rendered_from: [null, '出力の元'], evaluates: [null, '評価の対象'], about: [null, '対象'], declared_in: [null, '宣言の元'],
    convention_of: [null, '同じ規約'], defined_by: [null, '定義の元'], computed_from: [null, '計算の元'], mates_at: [null, '取合い先'], interface_feature: [null, '取合いの形状'], positioned_by: [null, '位置決めの元'],
    pattern_of: [null, '展開の対象'], uses: [null, '使用'],
  },
  lesson: { corrected_value: [null, '訂正された値'], engine_failure: [null, '検証で却下された結果'], integrity_failure: [null, '完全性チェックに失敗した結果'] },
  audit: {
    login: [null, 'ログイン'], logout: [null, 'ログアウト'], login_failed: [null, 'ログイン失敗'], setup: [null, '初回セットアップ'], access_denied: [null, 'アクセス拒否'],
    password_changed: [null, 'パスワード変更'], password_change_failed: [null, 'パスワード変更の失敗'], user_created: [null, 'ユーザー作成'], user_activated: [null, 'ユーザーの再有効化'], user_deactivated: [null, 'ユーザーの無効化'],
    member_added: [null, 'メンバー追加'], member_removed: [null, 'メンバー削除'], project_created: [null, 'プロジェクト作成'], upload_accepted: [null, 'アップロード受付'], upload_rejected: [null, 'アップロード拒否'],
    calculation: [null, '計算'], calculation_refused: [null, '計算の入力拒否'], correction: [null, '訂正'], correction_refused: [null, '訂正の拒否'], lesson_recorded: [null, '教訓の記録'],
    advisory_acknowledged: [null, '注意喚起の確認'], obligation_resolved: [null, '要精査項目の解消'], replay_audit: [null, '再実行監査'], replay_unavailable: [null, '再実行監査を実行できず'],
    dxf_export: [null, 'DXFのダウンロード'], stored_file_damaged: [null, '保存ファイルの破損'],
  },
  security: {
    allowed_hosts: [null, '応答するホスト名'], anti_forgery: [null, 'リクエスト偽造への対策'], audit_log: [null, '監査ログ'], blob_encryption: [null, '保存ファイルの暗号化'],
    client_address_source: [null, 'クライアントアドレスの取得元'], cookie_secure_flag: [null, '`Cookie`の`Secure`属性'], database_encrypted_at_rest: [null, 'データベースの保存時暗号化'],
    first_run_setup: [null, '初回セットアップ'], key_rotation: [null, '鍵のローテーション'], login_throttling: [null, 'ログイン試行の制限'], multi_factor_authentication: [null, '多要素認証'],
    password_hashing: [null, 'パスワードのハッシュ化'], project_isolation: [null, 'プロジェクト間の分離'], sandboxed_parsers: [null, 'パーサーのサンドボックス化'],
    session_lifetime_hours: [null, 'セッションの有効時間（時間）'], storage_key_outside_data_directory: [null, 'ストレージ鍵をデータディレクトリの外に置く'], transport: [null, '通信路'],
  },
  // names the engine gives to records in the dependency graph
  dlabel: { 'header-branch joint': [null, '母管と枝管の取合い'], intersection_curve: [null, '相貫線'], surface_reference: [null, '半径の基準面'] },
};
// Label of an identifier in the active language, or null when there is none (the caller shows the identifier).
export const idLabel = (kind, id) => { const e = ID[kind] && ID[kind][id]; return (e && e[cur === 'ja' ? 1 : 0]) || null; };
export const quantityUnit = (k) => (ID.quantity[k] ? ID.quantity[k][2] : null);
export const ID_TABLE = ID;

// ---------------------------------------------------------------- prose the engine and the server write
// A text is translated only if it is made of known templates from its first to its last character.
// Placeholders in the English template: {n} a number exactly as the engine printed it, {w} an identifier,
// {t} any text (a user's reason, an engine identifier with spaces). The Japanese template takes them by
// position, {1} {2} ..., and they are inserted unchanged. A third entry names the topic of a sentence
// (used to group the statements of gate G02).
const SENT = [
  // G01
  ['Parameters were entered manually; they are not linked to confirmed dimensions on a source drawing (drawing intelligence is not implemented).', 'パラメータは手入力されたもので、元図面上の確認済みの寸法とは連携していません（図面の読み取りは未実装）。'],
  // G02: open ambiguities
  ['Marking side: every pattern of this job is drawn as seen from the OUTSIDE of the formed part (pattern normal toward the outside).', 'けがき面：このジョブの展開図はすべて、成形後の部材を外面側から見た図として描かれています（展開図の法線は外面側を向きます）。', 'mside'],
  ['Spec 02 §0.3 requires a declared marking side but does not choose one; this side follows from the frames the specification writes down (§4.1, §4.4) and is a PROVISIONAL convention declared by the software.', '仕様書02 §0.3は、けがき面を宣言することを求めていますが、どちらの面とするかは定めていません。この面は、仕様書に書かれた座標系（§4.1、§4.4）から導いたもので、ソフトウェアが宣言した暫定の規約です。', 'mside'],
  ['It has not been reviewed by a fabrication engineer.', '製作技術者によるレビューは受けていません。', 'mside'],
  ['Marking a pattern on the other face mirrors the part (misfit of tens of millimetres for an offset, oblique branch).', '反対の面にけがくと、部材は鏡像になります（偏心した斜めの枝管では、数十ミリメートルの食い違いになります）。', 'mside'],
  ['An engineer must confirm the marking side before use.', '使用する前に、技術者がけがき面を確認する必要があります。', 'mside'],
  ['The parts of this job are not all drawn from the declared marking side ({t}); marked on one face they would not fit.', 'このジョブの部材は、すべてが宣言されたけがき面から描かれているわけではありません（`{1}`）。同じ面にけがくと、部材どうしが合いません。', 'mside'],
  ["Surface reference declared by the user as '{w}'.", '半径の基準面は、ユーザーにより「`{1}`」と宣言されています。', 'sref'],
  ['Surface reference of the entered radii is declared UNKNOWN: inside / outside / neutral must be established by an engineer before use.', '入力した半径の基準面は「不明（`UNKNOWN`）」と宣言されています。内面・外面・中立面のいずれであるかを、使用する前に技術者が確定する必要があります。', 'sref'],
  ['The declaration is recorded, not checked against a drawing.', 'この宣言は記録されるだけで、図面との照合は行われていません。', 'sref'],
  ['Contact and development radii were checked against the declared surface of each part (Spec 02 §6.1): the direction is possible; no wall thickness is modelled, so its size is not checked.', '取合い半径と展開半径を、各部材について宣言された基準面に照らして確認しました（仕様書02 §6.1）。大小の向きは成り立ち得ます。板厚はモデル化していないため、その大きさは確認していません。', 'radii'],
  ['{w} = {n} equals the outside contact radius {w} = {n}: that is a wall of zero thickness (an idealised surface).', '`{1}` = `{2}` は、外面の取合い半径 `{3}` = `{4}` と等しくなっています。これは板厚ゼロの壁（理想化された面）を意味します。', 'radii'],
  ['{w} = {n} equals the inside contact radius {w} = {n}: that is a wall of zero thickness (an idealised surface).', '`{1}` = `{2}` は、内面の取合い半径 `{3}` = `{4}` と等しくなっています。これは板厚ゼロの壁（理想化された面）を意味します。', 'radii'],
  ['Thickness is not modelled; an engineer must confirm the radii before use.', '板厚はモデル化していません。使用する前に、技術者が半径を確認する必要があります。', 'radii'],
  ['The relation between {w} = {n} and {w} = {n} could not be checked because the surface reference is unknown.', '基準面が不明のため、{1} = {2} と {3} = {4} の関係は確認できませんでした。', 'radii'],
  // G05
  ['Development class assigned by the surface-type constructor.', '展開クラスは、曲面の種類ごとの構築処理によって割り当てられました。'],
  ['Class naming scheme is a proposal (Spec 02 §14), not adopted.', 'クラスの命名体系は提案の段階（仕様書02 §14）で、採用されたものではありません。'],
  // G06
  ['No calibrated process data exists.', '校正済みの工程データはありません。'],
  ['Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld gap and kerf are not modelled.', '半径は、入力値をそのまま基準面の半径として使用します。板厚、各種の代（しろ）、溶接ギャップ、カーフは考慮していません。'],
  // G08
  ['eps_num is an a-priori rounding bound, not interval-certified.', '`eps_num`は丸め誤差の先験的な上界であり、区間演算による保証はありません。'],
  ['For an eccentric reducer its quadrature term is an ESTIMATE, not a bound.', '偏心レデューサでは、その求積項は推定値であり、上界ではありません。'],
  ['eps_rep is a certified upper bound (interval arithmetic, TOLERANCE_POLICY section 8) for a branch saddle and a header opening, where the pattern states it certified.', '`eps_rep`は、分岐の斜め部分と開口部について、認証された上界です（区間演算。許容誤差の方針文書 第8節）。'],
  ['For an eccentric reducer eps_rep is NOT certified: checks that would pass with it are INCONCLUSIVE; a FAIL stays FAIL.', '偏心レデューサの`eps_rep`は認証されていません。これを用いると合格となる検査は判定不能になり、不合格は不合格のままです。'],
  ['eps_rep is a certified upper bound of the chord error of the drawn polyline, eps_num included (interval arithmetic on the closed-form derivatives, docs/backend/TOLERANCE_POLICY.md section 8.1).', '`eps_rep`は、描かれた折れ線の弦の誤差の認証済みの上界であり、`eps_num`を含みます（導関数の閉形式に対する区間演算。許容誤差の方針文書 第8.1節）。'],
  ['eps_rep is NOT certified for this pattern (no bound could be proved, or its vertex term carries the quadrature estimate of the eccentric reducer): checks that would pass with it are INCONCLUSIVE (docs/backend/TOLERANCE_POLICY.md section 8.2).', 'このパターンの`eps_rep`は認証されていません（上界を証明できなかったか、その頂点項に偏心レデューサの求積の推定値が含まれるため）。これを用いると合格となる検査は判定不能になります（許容誤差の方針文書 第8.2節）。'],
  // G10
  ['Independent-formulation checks (class I-F), including interface constraints where the job has several parts.', '独立した定式化による検証項目（クラスI-F）です。ジョブに複数の部材がある場合は、取合いの拘束条件を含みます。'],
  ['No separately implemented verifier or external kernel cross-check exists yet.', '別に実装された検証器や、外部カーネルとの突き合わせは、まだありません。'],
  // G11
  ['Outer profiles closed, simple, counter-clockwise; openings closed, simple, clockwise (binary64 arithmetic, not exact predicates).', '外形は閉じた単純な輪郭で反時計回り、開口は閉じた単純な輪郭で時計回りです（`binary64`演算による判定で、厳密な述語ではありません）。'],
  ['Kerf-compensation ownership is not declared.', 'カーフ補正をどちらが行うかは、宣言されていません。'],
  // G14
  ['Algorithms are evidence level EV-1 (AI-generated, not independently reviewed) and not production-qualified.', 'アルゴリズムは証拠レベルEV-1（AIが生成、独立したレビューなし）で、製造用途としての認定は受けていません。'],
  // G15
  ['Every DXF re-imported by the independent reader and compared with the intended geometry.', 'すべてのDXFを独立したリーダーで再読込し、意図した形状と照合しました。'],
  ['CAM acceptance not tested.', '`CAM`での受入は試験していません。'],
  // G16
  ['Engineer approval workflow is not implemented.', '技術者承認のワークフローは未実装です。'],
  ['This result cannot be released for manufacturing.', 'この結果を製造用にリリースすることはできません。'],
  // G18
  ['{n} of {n} mandatory scrutiny obligations are OPEN.', '必須の要精査項目{2}件のうち、{1}件が未解決です。'],
  ["They cannot be closed by budget or time, only by evidence or an engineer's decision.", 'これらは、予算や時間の都合では解消できません。解消できるのは、証拠または技術者の判断だけです。'],
  ['All {n} scrutiny obligations are resolved.', '要精査項目{1}件は、すべて解決済みです。'],
  ['The propagation rule table is provisional ({t}), so closure cannot be certified.', '伝播の規則表は暫定版（`{1}`）のため、解消を保証することはできません。'],
  ['No scrutiny obligations recorded for this result.', 'この結果に対して記録された要精査項目はありません。'],
  ['The propagation rule table is provisional ({t}).', '伝播の規則表は暫定版（`{1}`）です。'],
  // G19
  ['No replay audit has been run for this result.', 'この結果に対して、再実行監査はまだ実行されていません。'],
  ['Audits A1-A4 are not implemented.', '監査A1-A4は未実装です。'],
  ['The stored replay report is unreadable.', '保存されている再実行の報告を読み取れません。'],
  ['The result is rejected until a replay audit succeeds.', '再実行監査が成功するまで、この結果は却下となります。'],
  ['An earlier replay attempt could not be run (infrastructure failure, not a finding about this result).', '以前の再実行は実行できませんでした（基盤側の障害であり、この結果についての所見ではありません）。'],
  ['Run the replay audit again.', '再実行監査をもう一度実行してください。'],
  ['Replay audit ({w}) reproduced the release closure.', '再実行監査（`{1}`）で、リリース閉包が再現されました。'],
  ['Audits A1-A4 are not implemented, and the replay used the same machine and build.', '監査A1-A4は未実装です。また、再実行には同じマシンと同じビルドを使用しました。'],
  ['Replay audit did NOT reproduce the stored result.', '再実行監査で、保存済みの結果は再現されませんでした。'],
  ['The result is rejected and must be investigated.', 'この結果は却下となり、調査が必要です。'],
  // G20
  ['{n} advisory(ies) open: this job repeats something that went wrong before in this project.', '未確認の注意喚起が{1}件あります。このジョブは、このプロジェクトで以前に誤りとなった内容を繰り返しています。'],
  ['An engineer must acknowledge each with a reason.', '技術者が、それぞれを理由を付けて確認する必要があります。'],
  ['{n} advisory(ies) raised and acknowledged by an engineer.', '注意喚起{1}件が発生し、技術者が確認済みです。'],
  ['No match among the {n} lesson(s) recorded in this project.', 'このプロジェクトに記録された教訓{1}件の中に、一致するものはありません。'],
  ['Lessons cover only mistakes already seen here.', '教訓が対象とするのは、ここで既に起きた誤りだけです。'],
  // G00
  ['Superseded by job #{n} after a correction.', '訂正により、ジョブ #{1} に置き換えられました。'],
  ['This result must not be used.', 'この結果を使用してはなりません。'],
  // assumptions
  ['Radii are taken as reference-surface (neutral) radii exactly as entered; thickness, neutral-factor, weld gap, shrinkage and kerf are not modelled.', '半径は、入力値をそのまま基準面（中立面）の半径として扱います。板厚、中立軸係数、溶接ギャップ、収縮、カーフは考慮していません。'],
  ['Mathematical model from Specification 02 (provisional; not independently reviewed; evidence level EV-1).', '数学モデルは仕様書02によります（暫定版。独立したレビューなし。証拠レベルEV-1）。'],
  ['Marking side: drawn as seen from the OUTSIDE of the formed part.', 'けがき面：成形後の部材を外面側から見た図です。'],
  ['Marking side: drawn as seen from the INSIDE of the formed part.', 'けがき面：成形後の部材を内面側から見た図です。'],
  ['Provisional convention of this build (Spec 02 §0.3 requires a declared side and does not choose one); not reviewed by a fabrication engineer.', 'このビルドの暫定規約です（仕様書02 §0.3は面の宣言を求めていますが、どちらの面とするかは定めていません）。製作技術者によるレビューは受けていません。'],
  ['Wall inclination varies around an oblique cone; a uniform reference-surface offset is a simplification (Spec 02 research item R-05).', '斜円錐では、壁の傾きが周方向で変わります。基準面のオフセットを一様とするのは簡略化です（仕様書02 研究項目R-05）。'],
  ['Seam placed at theta = 0; no seam-placement rule applied.', '継目は `theta` = 0 の位置に置いています。継目の位置に関する規則は適用していません。'],
  ['Saddle cut = the curve where the branch surface of the entered contact radius meets the header surface of the entered contact radius (Spec 02 §4.4).', '相貫切断線＝入力した取合い半径による枝管の面と、入力した取合い半径による母管の面が交わる曲線です（仕様書02 §4.4）。'],
  ['The cut is square through the wall (constant height through the thickness, as a cut perpendicular to the flat plate gives). Weld preparation, bevel and root gap are not modelled.', '切断は板厚を貫く直角の切断です（平板に垂直な切断が与える、厚さ方向で一定の高さ）。開先加工、ベベル、ルートギャップは考慮していません。'],
  ['Weld preparation and root gap are not modelled.', '開先加工とルートギャップは考慮していません。'],
  ['Polyline density: {n} segments per curve, uniform in the angle; doubled from the base number until the measured chord error is within the chord error limit.', 'ポリラインの密度：曲線1本あたり{1}セグメント。角度について一様で、測定した弦誤差が弦誤差の上限に収まるまで、基本数から倍にしています。'],
  ['Chord error limit: {n} mm, an implementation limit pending engineering review (docs/backend/TOLERANCE_POLICY.md); not an engineering tolerance.', '弦誤差の上限：{1} mm。工学的な公差ではなく、工学的レビュー待ちの実装上の上限です（`docs/backend/TOLERANCE_POLICY.md`）。'],
  ['Joint not declared: the saddle and the opening are cut on one curve of the entered contact radii, a zero-thickness idealisation.', '取合いの形式は未宣言です。相貫切断線と開口は、入力した取合い半径による1本の曲線で切断しています。板厚をゼロとした理想化です。'],
  ['A set-on branch is cut at its bore: declare the joint as set-on (request value set_on) with header_surface_reference outside and branch_surface_reference inside.', 'セットオンの枝管は、その内径で切断します。セットオンとするには、取合いを`set_on`として宣言し、母管の基準面を`outside`、枝管の基準面を`inside`としてください。'],
  ['Every DXF is re-imported by a separate reader and compared with the intended geometry, and checked by the DXF quality gate (open contours, overlaps, self-intersection, orientation, extents, release marking); a file that fails is not offered.', 'すべてのDXFを別のリーダーで再読込し、意図した形状と照合します。また、DXF品質ゲート（開いた輪郭、重なり、自己交差、向き、外形寸法、リリース表示）で検査します。不合格のファイルは提供しません。'],
  ['Seam at branch azimuth phi = 0.', '継目は、枝管の方位角 `phi` = 0 の位置です。'],
  ['Header rolled from plate; longitudinal seam placed diametrically opposite the branch.', '母管は、板を巻いて製作する前提です。長手継手は、枝管の正反対の位置に置いています。'],
  ['No seam-to-opening clearance rule is applied.', '継目と開口の離れに関する規則は適用していません。'],
  ['Opening = the curve where the branch surface of the declared radius meets the header surface of the declared radius (Spec 02 §4.4), cut square through the wall.', '開口＝宣言した半径による枝管の面と、宣言した半径による母管の面が交わる曲線です（仕様書02 §4.4）。切断は板厚を貫く直角の切断です。'],
  ['Which wall surfaces these are and the joint type are declared by the user and recorded with the job; weld preparation, root gap and bevel are not modelled.', 'これらがどの壁面かと取合いの形式は、ユーザーが宣言し、ジョブとともに記録したものです。開先加工、ルートギャップ、ベベルは考慮していません。'],
  ['Which wall surfaces these are, and the joint type, are declarations of the user recorded with the job.', 'これらがどの壁面かと、取合いの形式は、ユーザーが宣言し、ジョブとともに記録したものです。'],
  // replay audit: stated limitation
  ['Replay ran in a separate process that received only the request, on the same machine with the same build (the child reported the same engine identifier).', '再実行は、リクエストだけを受け取る別プロセスで、同じマシン・同じビルドで行いました（子プロセスは同じエンジン識別子を報告しました）。'],
  ['A separately built binary or separate machine is not used.', '別にビルドしたバイナリや、別のマシンは使用していません。'],
  ['Discrepancies are reported, never repaired.', '不一致は報告されるだけで、修復されることはありません。'],
  ['This audit shows reproducibility, not correctness.', 'この監査が示すのは再現性であり、正しさではありません。'],
  // audit chain: stated limitation
  ['Detects alteration or removal of entries inside the chain, and removal of the newest entries: the latest hash, the entry count and the latest id are stored as an anchor authenticated with the storage key.', 'チェーン内のエントリーの改変と削除、および最新のエントリーの削除を検出します。最新のハッシュ、エントリー数、最新のIDを、ストレージ鍵で認証したアンカーとして保存しています。'],
  ['Someone who can write the database file AND read the storage key can rebuild the chain and the anchor without trace.', 'データベースファイルへの書き込みと、ストレージ鍵の読み取りの両方ができる者は、痕跡を残さずにチェーンとアンカーを作り直せます。'],
  ['Someone who can write the database file can also restore an older copy of the whole database together with its older anchor (rollback); that is not detected.', 'データベースファイルに書き込める者は、古いアンカーを含むデータベース全体の古いコピーを復元すること（ロールバック）もできます。これは検出されません。'],
  ['An anchor held outside this machine is not implemented.', 'このマシンの外にアンカーを保持する機能は、未実装です。'],
];

const WHOLE = [
  // interface record, joint (engine 0.4.0)
  ['set-on (Spec 02 section 4.4: branch inside surface on header outside surface; the opening is the branch bore)', 'セットオン（仕様書02 4.4節：母管の外面に枝管の内面。開口は枝管の内径）'],
  ['not declared (the branch end cut and the header opening are cut on one curve of the entered radii)', '取合いの形式は未宣言（枝管の端部の切断と母管の開口を、入力した半径による1本の曲線で切断）'],
  // verification methods
  ['circumference 2*pi*r from the input radius', '入力した半径から求めた円周長 2*pi*r'],
  ['length of the opposite seam edge', '反対側の継目の辺の長さ'],
  ['input length', '入力した長さ'],
  ['lateral area 2*pi*r*L', '側面積 2*pi*r*L'],
  ['3D end-circle circumference 2*pi*r1', '3Dの端円の円周長 2*pi*r1'],
  ['3D end-circle circumference 2*pi*r2', '3Dの端円の円周長 2*pi*r2'],
  ['slant length from Pythagoras on the axial section', '軸を含む断面での三平方の定理による母線長'],
  ['lateral area pi*(r1+r2)*s of the 3D frustum', '3Dの円錐台の側面積 pi*(r1+r2)*s'],
  ['implicit taper equation of the frustum evaluated at folded-back arcs', '立体に戻した円弧上で評価した、円錐台のテーパーの陰関数式'],
  ['pattern structure', '展開図の構成'],
  ['geodesic-curvature integration of the large end circle (no apex), RK4', '大径側の端円の測地的曲率の積分（頂点を使わない）、`RK4`'],
  ['3D circumference 2*pi*r1 (polyline chord shortening allowed for)', '3Dの円周長 2*pi*r1（ポリラインの弦による短縮を見込む）'],
  ['3D circumference 2*pi*r2 (polyline chord shortening allowed for)', '3Dの円周長 2*pi*r2（ポリラインの弦による短縮を見込む）'],
  ['distance between the 3D end-circle points at theta = 0', '`theta` = 0 における、3Dの両端円上の点の距離'],
  ['surface integral of |S_theta x S_lambda| over the 3D ruled surface', '3Dの線織面上での |`S_theta` x `S_lambda`| の面積分'],
  ['fold-back by the declared marking side, then distance from the header axis (implicit equation)', '宣言されたけがき面に従って立体に戻し、母管軸からの距離を評価（陰関数式）'],
  ['fold-back by the declared marking side, then |(P-O) x d| distance from the branch axis', '宣言されたけがき面に従って立体に戻し、枝管軸からの距離 |(P-O) x d| を評価'],
  ['s(270 deg) - s(90 deg) read from the pattern through the declared marking side against 2*r*cot(beta)', '宣言されたけがき面に従って展開図から読み取った `s(270 deg) - s(90 deg)` を、`2*r*cot(beta)` と比較'],
  ['sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)', '外形の符号付き面積の符号。外形は、展開図の座標系で反時計回りです（仕様書02 0.3節）'],
  ['circumference 2*pi*r_dev', '円周長 `2*pi*r_dev`'],
  ['largest x of the plate outline against the input header length', '板の外形のx最大値を、入力した母管の長さと比較'],
  ['largest y of the plate outline against 2*pi*r from the input radius', '板の外形のy最大値を、入力した半径から求めた 2*pi*r と比較'],
  ['fold-back onto the header by the declared marking side, then |(P-O) x d| distance from the branch axis', '宣言されたけがき面に従って母管上に戻し、枝管軸からの距離 |(P-O) x d| を評価'],
  ['profile test of the opening and bounds test against the shell rectangle', '開口の輪郭の検査と、胴板の長方形に対する範囲の検査'],
  ['declared marking side of the header pattern against that of the branch pattern (1 = outside, -1 = inside)', '母管の展開図で宣言されたけがき面を、枝管の展開図のものと比較（1 = 外面、-1 = 内面）'],
  ['3D distance from each folded-back branch cut point to the folded-back header opening curve', '立体に戻した枝管切断線の各点から、立体に戻した母管の開口曲線までの3D距離'],
  ['3D distance from each folded-back opening point to the folded-back branch cut curve', '立体に戻した開口の各点から、立体に戻した枝管の切断曲線までの3D距離'],
  // scrutiny obligations: required checks (provisional rule table) and the engine's notes
  ['Recompute the intersection curve from the changed parameters', '変更されたパラメータから、相貫線を再計算する'],
  ['Recompute the development and pass family verification and the profile check', '展開を再計算し、形状種別ごとの検証と輪郭の検査に合格する'],
  ['Regenerate the DXF and pass the independent re-import check', 'DXFを再出力し、独立した再読込・照合に合格する'],
  ['Pass the interface check: both parts must reproduce the same joint curve', '取合いの確認に合格する。両方の部材が、同じ取合い線を再現すること'],
  ['Re-check the fit of this part at the joint', 'この部材の、取合い部での合いを再確認する'],
  ['Engineer to re-confirm this value: it was entered under the same convention as the corrected one', '技術者がこの値を再確認する。訂正された値と同じ規約のもとで入力されたため'],
  ["requires an engineer's decision", '技術者の判断が必要'],
  ["recomputed; verification checks for '{w}' and the profile check pass", '再計算済み。「`{1}`」の検証項目と輪郭の検査に合格'],
  ['regenerated; independent re-import check passes', '再出力済み。独立した再読込・照合に合格'],
  ['recomputed; both parts reproduce the same joint curve', '再計算済み。両方の部材が同じ取合い線を再現'],
  ['interface check passes on the recomputed parts', '再計算した部材で、取合いの確認に合格'],
  ['no passing evidence in the recomputed job', '再計算したジョブに、合格を示す証拠がありません'],
  // advisories
  ['{w} = {n} repeats a value that was corrected in this project (job #{n}: {n} -> {n}; reason: {t}). Confirm that {n} is right this time.', '{1} = {2} は、このプロジェクトで訂正された値の繰り返しです（ジョブ #{3}：{4} -> {5}、理由：{6}）。今回は {7} で正しいことを確認してください。'],
  ['This exact request produced a result rejected by verification before in this project (job #{n}).', 'これと全く同じリクエストは、このプロジェクトで以前、検証で却下される結果になりました（ジョブ #{1}）。'],
  ['This exact request produced a stored result that failed its integrity check before in this project (job #{n}).', 'これと全く同じリクエストは、このプロジェクトで以前、保存結果が完全性チェックに失敗しました（ジョブ #{1}）。'],
  // document storage
  ['not implemented: the file is stored encrypted and hashed; no drawing content is read', '読み取りは未実装です。ファイルは暗号化し、ハッシュ化して保存されます。図面の内容は読み取りません'],
  // AMOEBA mechanisms
  ['content-addressed append-only records', '内容アドレス方式・追記専用の記録'],
  ['project snapshots with predecessor links', '直前のスナップショットへのリンクを持つ、プロジェクトスナップショット'],
  ['typed dependency edges (derivation, interface, common cause)', '型付きの依存関係エッジ（導出、取合い、共通原因）'],
  ['dependency propagation with scrutiny obligations (provisional rule table)', '要精査項目を伴う依存関係の伝播（暫定の規則表）'],
  ['interface constraint verification between parts', '部材間の取合い確認'],
  ['lessons from corrections and failures, matched against every later job in the project', '訂正と不合格から得た教訓（プロジェクト内の以降のすべてのジョブと照合）'],
  ['corrections as attestations with supersession', '証明としての訂正（旧版化を伴う）'],
  ['release closure hash', 'リリース閉包のハッシュ'],
  ['gate evaluation with obligation and replay state', '要精査項目と再実行の状態を反映したゲート評価'],
  ['replay audit in a separate process', '別プロセスでの再実行監査'],
  ['membrane admission checks', 'メンブレン（受入境界）での受入チェック'],
  ['round scheduler and budgets', 'ラウンドのスケジューラと予算'],
  ['exploration probes', '探索プローブ'],
  ['hypothesis-driven investigation', '仮説に基づく調査'],
  ['coverage ledger', 'カバレッジ台帳'],
  ['lanes and overlays', 'レーンとオーバーレイ'],
  ['quarantine', '隔離'],
  ['strategy switching', '戦略の切り替え'],
  ['self-consistency audits A1-A4', '自己整合性監査A1-A4'],
  ['replay on a separate machine or build', '別のマシンまたはビルドでの再実行'],
  ['engineer approval', '技術者承認'],
];

// Reasons the engine gives when it refuses a request. The outcome is "<class> (<parameter>): <reason>".
const REASON = [
  ['must be finite and > 0, got {n}', '有限で0より大きい値が必要です（入力値 {1}）'],
  ['must be finite', '有限の値が必要です'],
  ['requires r1 > r2 (large end first); equal radii is a cylinder', 'r1 > r2 が必要です（大径側を先に）。半径が等しい場合は円筒です'],
  ['near-cylindrical cone: apex-based formula is ill-conditioned and the apex-free method is not implemented in this build', '円筒に近い円錐です。頂点を基準とする式は悪条件になり、頂点を使わない方法は、このビルドでは未実装です'],
  ['offset must be finite and >= 0', '偏心量は、有限で0以上の値が必要です'],
  ['requires r1 > r2; equal radii with offset (oblique cylinder) is not implemented in this build', 'r1 > r2 が必要です。半径が等しく偏心がある場合（斜円筒）は、このビルドでは未実装です'],
  ['near-cylindrical: apex-free method not implemented in this build', '円筒に近い形状です。頂点を使わない方法は、このビルドでは未実装です'],
  ['angle between axes must be within about 10..170 deg in this build (grazing intersections are ill-conditioned; limit is an implementation choice, not an engineering rule)', 'このビルドでは、両軸のなす角はおよそ10〜170度の範囲が必要です（浅い角度の相貫は悪条件になります。この制限は実装上の選択であり、技術上の規則ではありません）'],
  ['requires |delta| + r < R: tangent and partial intersections (including the equal-diameter tee) are not implemented in this build', '`|delta| + r` < `R` が必要です。接する場合と部分的な相貫（同径のT継手を含む）は、このビルドでは未実装です'],
  ['square end must lie beyond the saddle: s_end > {n}', '直切り端は、相貫切断線より先にある必要があります：`s_end` > {1}'],
  ['branch geometry outside the supported range (see branch stub limits)', '枝管の形状が対応範囲の外です（枝管スタブの制限を参照）'],
  ['opening spans x = {n} .. {n} and does not lie inside the shell length {n}', '開口は x = {1} .. {2} にわたり、胴の長さ {3} の範囲に収まっていません'],
  ['magnitude {n} mm exceeds {n} mm, the largest size this build has been exercised with (implementation limit, not an engineering rule)', '大きさ {1} mm が、このビルドで動作を確かめた最大寸法 {2} mm を超えています（実装上の制限であり、技術上の規則ではありません）'],
  ['{n} mm is below {n} mm, the smallest dimension this build has been exercised with (implementation limit, not an engineering rule)', '{1} mm は、このビルドで動作を確かめた最小寸法 {2} mm を下回っています（実装上の制限であり、技術上の規則ではありません）'],
  ['developed angle {n} rad would overlap', '展開角 {1} rad では、展開図が重なります'],
];

// Messages the server answers a refused request with (the `error` field). Others stay in English, marked.
const SERVER = [
  ['invalid username or password', 'ユーザー名またはパスワードが正しくありません'],
  ['not signed in', 'ログインしていません'],
  ['not found', '見つかりません'],
  ['your role does not permit this action', 'この権限では、この操作はできません'],
  ['username must be 3-64 characters: letters, digits, . _ -', 'ユーザー名は3〜64文字で、使えるのは英字、数字、. _ - です'],
  ['role must be admin, engineer or viewer', '権限は `admin`、`engineer`、`viewer` のいずれかにしてください'],
  ['username already exists', 'このユーザー名は既に使われています'],
  ['setup already completed', '初回セットアップは既に完了しています'],
  ['too many setup attempts with a wrong token; try again in 10 minutes', '誤ったトークンでのセットアップ試行が多すぎます。10分後にもう一度お試しください'],
  ['setup token required: the first administrator can only be created with the one-time setup token of this installation (shown to the installer at first start, or set with NF_SETUP_TOKEN)', 'セットアップトークンが必要です。最初の管理者は、このインストールの1回限りのセットアップトークン（初回起動時にインストールした人に表示されるか、`NF_SETUP_TOKEN`で設定したもの）がなければ作成できません'],
  ['too many failed sign-in attempts for this account; try again in 15 minutes', 'このアカウントでのログイン失敗が多すぎます。15分後にもう一度お試しください'],
  ['too many failed sign-in attempts from this address; try again in 10 minutes', 'このアドレスからのログイン失敗が多すぎます。10分後にもう一度お試しください'],
  ['too many wrong current passwords; try again in 15 minutes', '現在のパスワードの誤りが多すぎます。15分後にもう一度お試しください'],
  ['current password is incorrect', '現在のパスワードが正しくありません'],
  ['password must have at least 10 characters', 'パスワードは10文字以上にしてください'],
  ['new password must have at least 10 characters', '新しいパスワードは10文字以上にしてください'],
  ['password is too long (at most 1024 bytes)', 'パスワードが長すぎます（最大1024バイト）'],
  ['new password is too long (at most 1024 bytes)', '新しいパスワードが長すぎます（最大1024バイト）'],
  ['password must not be the user name', 'パスワードをユーザー名と同じにすることはできません'],
  ['new password must not be the user name', '新しいパスワードをユーザー名と同じにすることはできません'],
  ['this password is on the list of commonly used passwords; choose another', 'このパスワードは、よく使われるパスワードの一覧にあります。別のものにしてください'],
  ['new this password is on the list of commonly used passwords; choose another', 'このパスワードは、よく使われるパスワードの一覧にあります。別のものにしてください'],
  ['you cannot deactivate your own account', '自分自身のアカウントを無効化することはできません'],
  ['project name is required', 'プロジェクトの名称を入力してください'],
  ['project name is too long (max 200 characters)', 'プロジェクトの名称が長すぎます（最大200文字）'],
  ['file exceeds the upload limit of {n} MiB', 'ファイルが、アップロードの上限 {1} `MiB` を超えています'],
  ['too many uploads are in progress; retry shortly', '進行中のアップロードが多すぎます。少し待ってから再試行してください'],
  ['the upload was interrupted', 'アップロードが中断されました'],
  ['file rejected at the membrane: empty', 'ファイルは受け付けられませんでした：中身が空です'],
  ['file rejected at the membrane: too_large', 'ファイルは受け付けられませんでした：大きすぎます'],
  ['file rejected at the membrane: unsupported_format', 'ファイルは受け付けられませんでした：対応していない形式です'],
  ['a reason is required', '理由が必要です'],
  ['a reason is required for every correction', '訂正には必ず理由が必要です'],
  ['advisory is not open', 'この注意喚起は、未確認の状態ではありません'],
  ['obligation is not open', 'この要精査項目は、未解決の状態ではありません'],
  ['nothing to replay: the request was refused and nothing was computed', '再実行するものがありません。リクエストは受け付けられず、何も計算されていません'],
  ['this result is already superseded; correct the current one', 'この結果は既に旧版です。現行の結果を訂正してください'],
  ['this request was refused and has no result to correct; submit a new calculation', 'このリクエストは受け付けられておらず、訂正する結果がありません。新しい計算を実行してください'],
  ['corrections with dependency propagation are implemented for assembly jobs only', '依存関係の伝播を伴う訂正は、組立のジョブでのみ実装されています'],
  ['new value equals the current value', '新しい値が現在の値と同じです'],
  ['new_value must be a finite number', '新しい値は有限の数値にしてください'],
  ['unknown parameter', '不明なパラメータです'],
  ['parameter cannot be corrected', 'このパラメータは訂正できません'],
  ['this result was superseded by job #{n}; its files are withheld', 'この結果は、ジョブ #{1} により旧版となりました。ファイルは提供されません'],
  ['this result is REJECTED; its files are withheld', 'この結果は却下されています。ファイルは提供されません'],
  ['no DXF for this part', 'この部材のDXFはありません'],
  ['no DXF for this part: the request was refused and nothing was computed', 'この部材のDXFはありません。リクエストは受け付けられず、何も計算されていません'],
  ['the database is busy; nothing was changed. Retry shortly.', 'データベースが使用中です。何も変更されていません。少し待ってから再試行してください。'],
  ["the server's storage is full; nothing was changed and nothing was recorded. An administrator must free space.", 'サーバーのストレージに空きがありません。何も変更されず、何も記録されていません。管理者が空き容量を確保する必要があります。'],
  ['internal storage error; nothing was changed. The server log has the details.', 'ストレージの内部エラーです。何も変更されていません。詳細はサーバーのログにあります。'],
  ['internal error; nothing was changed. The server log has the details.', '内部エラーです。何も変更されていません。詳細はサーバーのログにあります。'],
  ['the request was not completed in time and was abandoned', 'リクエストは時間内に完了せず、打ち切られました'],
  ['missing anti-forgery header', '偽造対策のヘッダーがありません'],
  ['cross-origin request refused', '別オリジンからのリクエストは拒否されました'],
];

const NUM = '(-?(?:\\d[\\d.]*(?:e[-+]?\\d+)?|NaN|inf))';
const compile = (tpl, whole) => new RegExp('^' + tpl.split(/(\{[nwt]\})/).map((s) => (s === '{n}' ? NUM : s === '{w}' ? '([A-Za-z_][\\w.+-]*)' : s === '{t}' ? '(.+?)' : s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))).join('') + (whole ? '$' : '(?=\\s|$)'));
const table = (rows, whole) => rows.map(([en, ja, topic]) => ({ re: compile(en, whole), ja, topic: topic || null }));
let T = null;
const tables = () => T || (T = { sent: table(SENT, false), whole: table(WHOLE, true), reason: table(REASON, true), server: table(SERVER, true) });
const subst = (ja, m) => ja.replace(/\{(\d)\}/g, (_, i) => m[Number(i)]);
const wholeJa = (rows, text) => { for (const r of rows) { const m = r.re.exec(text); if (m) return subst(r.ja, m); } return null; };

// "out of range (r1-r2): <reason>", "not developable: <reason>", "degenerate geometry: <reason>"
function outcomeJa(text) {
  const m = /^(out of range \(([^)]+)\)|not developable|degenerate geometry): (.+)$/.exec(text); if (!m) return null;
  const why = wholeJa(tables().reason, m[3]); if (!why) return null;
  return `${m[2] ? `範囲外（${m[2]}）` : m[1] === 'not developable' ? '展開不可' : '退化した形状'}：${why}`;
}
// Sentence by sentence. `ja` is null for a sentence that is not a known template; the caller must then
// show the WHOLE text in English (a text is never translated in part).
export function engineSentences(text) {
  const out = []; let rest = String(text ?? '').trim();
  const { sent } = tables();
  while (rest) {
    let hit = null;
    for (const r of sent) { const m = r.re.exec(rest); if (m) { hit = [r, m]; break; } }
    if (!hit) {
      // an unknown sentence: cut it at the next sentence end so that the following ones can still be listed in English
      const cut = /^.*?[.!?](?=\s+[A-Z0-9'"(]|$)/.exec(rest); const en = cut ? cut[0] : rest;
      out.push({ en, ja: null, topic: null }); rest = rest.slice(en.length).trimStart(); continue;
    }
    const [r, m] = hit; out.push({ en: m[0], ja: subst(r.ja, m), topic: r.topic }); rest = rest.slice(m[0].length).trimStart();
  }
  return out;
}
// The Japanese text of an engine or server text, or null if any part of it is not a known template.
export function engineJa(text) {
  const s = String(text ?? '').trim(); if (!s) return null;
  const { whole, server } = tables();
  let ja = wholeJa(whole, s) || outcomeJa(s) || wholeJa(server, s);
  if (ja) return ja;
  let m = /^No pattern produced: (.+)$/.exec(s); if (m) { const o = outcomeJa(m[1]); return o ? `展開図は生成されませんでした：${o}` : null; }
  m = /^request refused, nothing was computed: (.+)$/.exec(s); if (m) { const o = outcomeJa(m[1]); return o ? `リクエストは受け付けられず、何も計算されていません：${o}` : null; }
  m = /^correction refused, nothing was changed: ([A-Za-z_]\w*) = (\S+) cannot be computed \((.+)\)\. Job #(\d+) is still the current result\.$/.exec(s);
  if (m) { const o = outcomeJa(m[3]); return o ? `訂正は受け付けられず、何も変更されていません：${m[1]} = ${m[2]} は計算できません（${o}）。現行の結果は、引き続きジョブ #${m[4]} です。` : null; }
  const segs = engineSentences(s);
  return segs.length && segs.every((x) => x.ja) ? segs.map((x) => x.ja).join('') : null;
}
export const PROSE_TABLES = { SENT, WHOLE, REASON, SERVER };
