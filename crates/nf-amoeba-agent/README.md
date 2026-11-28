# nf-amoeba-agent: AMOEBA tool bridge

`nf-amoeba-tools` lets a model drive the running NEXUS FABRIC server through a fixed list of tools. The model decides
what to ask. Every number comes from the server's own code. The bridge never re-implements the engine, the digest or
the gates, and it cannot release, approve or override a result.

This is a development build. Outputs are not validated and must not be used for manufacturing.

## Start a session

The server runs in desktop mode on loopback (`nf-server --desktop --no-browser --port N`). The bridge needs a
dedicated engineer account on that server. Create it in the UI or with the API. Then:

```
NF_AGENT_USER=<account> NF_AGENT_PASS=<password> \
  nf-amoeba-tools --base http://127.0.0.1:<port> --steps /path/to/steps.jsonl --docs-dir /path/to/pdfs
```

- `--base` must be a loopback address (`127.0.0.1` or `localhost`). The bridge signs in once at start-up.
- The account must be an engineer account. The bridge refuses to start for an administrator account (it checks the
  role with `/api/me`) and exits non-zero.
- `--docs-dir` is required. Uploads may name only files inside it: symlinks are resolved first, a file of more than
  100 MB is refused, and a missing file and a file outside the directory get the same answer.
- If a step record cannot be written, the bridge stops with a non-zero exit and gives no answer for that call.
- The password is read from the environment, sent once to `/api/login`, and never printed or written to the steps file.
- `--steps` is append-only evidence. Step numbers continue across runs.

## Protocol

One JSON request per line on stdin. One JSON answer per line on stdout, in the same order.

```
-> {"id": 1, "tool": "calculate", "args": {"project": 1, "family": "cone_frustum", "params": {"r1": 200, "r2": 100, "h": 300}}}
<- {"id": 1, "ok": true, "step": 2, "result": {"id": 2, "state": "REVIEW_REQUIRED", "advisories": 0}}
```

- `ok: false` answers carry `error` instead of `result`. A refused request has `error: "tool not available"`.
- A server refusal (other 4xx) is data, not a crash: `result` is `{"http_status": N, "rejected": <server body>}`.
- A withheld DXF is data too: `get_dxf_digest` returns `{"held": true, "http_status", "hold": <server body unchanged>}`
  (HTTP 409 only) and no bytes or hash. Other refusals of a DXF request are rejections, not holds.

## Tools

The list and the function-calling schema come from the binary: `nf-amoeba-tools --schema` (a JSON array of
`{name, description, parameters}`). Validate arguments against it before calling. The bridge also checks every call
against the same schema and refuses unknown or missing names without sending anything.

Read and compute tools: `list_projects`, `get_project`, `upload_document` (PDF content only), `pdf_to_dxf`,
`get_conversion`, `calculate`, `get_job`, `get_amoeba`, `get_dependencies`, `replay`, `recompute`,
`list_obligations`, `list_lessons`, `amoeba_summary`, `get_dxf_digest`, `propose_read`.

Not available, by construction: corrections, obligation resolution, setup, account and membership changes, password
changes, sign-out of others, audit writes, and any approval or release. A call to any of these names returns
`tool not available` and makes no HTTP request.

`recompute` is a write: it supersedes the job and records an audit entry. Call it only when a new engine result is wanted.

`propose_read` records a CANDIDATE reading (document, page, box, value, unit, kind) in the steps file. It is never
sent to the server, and `calculate` never reads it. Every parameter of a calculation must be given explicitly.

## Steps file

One JSON line per call, including the sign-in (`tool: "login"`) and refused calls:

`step`, `ts_ms`, `tool`, `args` (secrets replaced by `[redacted]`), `status` (`ok`, `error`, `refused`, `CANDIDATE`),
`http_status` (list of statuses, empty when no request was sent), `result_sha256`, `duration_ms`, `engine_id`, `error`.

`result_sha256` is the SHA-256 of the answer's canonical JSON with wall-clock fields removed (keys `at`, `ts`, and keys
ending in `_at`). The model still receives those fields. With the stripping, a second run of the same script on the
same engine gives the same hashes (checked by `tests/session.rs`).

## Driving it from a model runner

Load the tool list from `--schema` as the model's tool definitions. For each model tool call, write one request line,
read one answer line, and return `result` or `error` to the model. Keep the steps file as the trajectory record. Do not
add tools that write decisions. The model may be wrong about values, but it cannot make the server accept them.

## Tests

- `cargo test -p nf-amoeba-agent --locked` runs the unit tests, the schema contract, the refusal test and the
  end-to-end session. The session starts the release `nf-server` (build it first with
  `cargo build --release -p nf-server -p nf-pdf`, or set `NF_SERVER_BIN`) on a loopback port from 8825 to 8829.
