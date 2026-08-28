# TASK-106 — fixed decisions (verbatim)

Planner-owned. Copied from the decomposition plan; numbering is global across tasks. Read-only. Anything not covered here that changes public behavior is `NEEDS_REPLAN`.

## D-002 Module layout (relevant part)

```text
crates/pgtui/src/
  main.rs           CLI (clap): --db <path>; terminal setup/teardown; runtime loop; exit codes (D-040)
  runtime.rs        execute(Effect) -> Msg against ConnectionStore + Option<PgSession>
  render.rs         PLANNER, protected: Buffer→text, Buffer→SVG, SVG→PNG (resvg + embedded DejaVu Sans Mono)
  bin/gallery.rs    render every screen to docs/screens/*.svg|png using fixture data
```

`crates/pgtui/Cargo.toml` already declares `[[bin]] gallery` (path `src/bin/gallery.rs`); the stub is replaced, not re-declared.

## D-012 Runtime loop

`main.rs`: parse CLI → open store (fail → exit 2) → enter raw mode/alternate screen → `App::update(Msg::Connections(store.list()))` → loop { draw; read key (blocking); effects = update(Key); for each effect: execute → feed reply Msg → collect further effects } until `Effect::Quit`. DB calls block the loop (no background tasks) — accepted; the app is single-user. `Effect::Disconnect` replies `Msg::Disconnected`; when it precedes `Quit` in the same effect list, it is executed first.

## D-024 Connection config

Built from fields, never typed by the user: `host=<host> port=<port> dbname=<dbname> user=<username> password=<password> application_name=pgtui` (tokio-postgres key/value config, `NoTls`). No sslmode, no URL parsing. Connect timeout 5 s (`tokio::time::timeout` around connect). Failure is non-fatal: `Msg::Connected(Err)` → status `error: ...`, screen stays `ConnectionList`. (`application_name=pgtui` is added by this task so `pg_disconnect_test` can count backends in `pg_stat_activity`.)

Disconnect: the runtime removes the `PgSession`, drops the `Client`, and awaits the `JoinHandle` of the spawned `Connection` future so the socket is closed before `Msg::Disconnected` is returned.

## D-030 Global keys

`Ctrl+C` → `Effect::Disconnect` (if connected) then `Effect::Quit`, from every screen. No other global key.

## D-033 Browser (disconnect part)

`d` → `Effect::Disconnect`; `Msg::Disconnected` → `session = None`, `grid = None`, `sql_grid = None`, `Screen::ConnectionList`. `list_cursor` and `connections` are unchanged. `sql_input` is cleared.

## D-040 Exit codes

`0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first. `std::process::exit` is called only from `main.rs` (never from `app.rs` or `runtime.rs`).

## D-041 Error surfacing

Every `Err` reaching `App::update` becomes `Status::Error`. No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## D-070 `tests/support/mod.rs` (planner-shipped, protected)

Thin wrappers over `pgtui::render`: `render_text(&App) -> String`, `render_svg(&App) -> String`, `svg_to_png(&str) -> Vec<u8>` (PNG is asserted for non-zero length and dimensions `(100*cw, 30*ch)`, never byte-compared); key constructors; `temp_store()`; `pg_container()` (`postgres:16-alpine` + `fixtures/seed.sql`); `fake_data::{tables(), preview(table) -> ResultSet}`.

## D-071 Snapshot policy and names

Text snapshots `insta::assert_snapshot!("<name>", render_text(&app))`, SVG `insta::assert_snapshot!("<name>__svg", render_svg(&app))`. All `.snap` files are protected. Gate runs with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; any `*.snap.new` fails the gate.

The ten gallery screens (fixed names):

1. `screen__connection_list_empty`
2. `screen__connection_list_two`
3. `screen__create_form_blank`
4. `screen__create_form_filled`
5. `screen__browser_sidebar_empty_body`
6. `screen__preview_unsorted`
7. `screen__preview_sorted_asc`
8. `screen__preview_sorted_desc`
9. `screen__custom_sql_empty`
10. `screen__custom_sql_results`

(`screen__create_form_saved_list` is an 11th text-only snapshot and is not part of the gallery.)

## D-080 `gallery` binary contract

- CLI (clap): `gallery [--out <dir>]`; default `docs/screens`. Creates `<dir>` with `create_dir_all`.
- For each of the ten D-071 names, build the `App` state exactly as the corresponding `screen_*_test.rs` builds it (same fake connections, same `fake_data` result sets, same cursor/sort/focus), render with `pgtui::render` at 100×30, and write `<dir>/<name>.svg` (the `render_svg` string, unchanged) and `<dir>/<name>.png` (the `svg_to_png` bytes).
- Output is deterministic: no timestamps, no random values, no environment-dependent paths in the SVG.
- Exit codes: `0` success; `2` bad arguments or unwritable output directory with `error: <msg>` on stderr; `1` render failure.
- The bin contains no rendering, font, or SVG code of its own (`R-005`). It may share state-construction helpers with tests only by re-implementing the tiny state builders locally; it must not `#[path]`-include anything under `tests/`.
- `docs/screens/` holds one committed run of `gallery` with the default `--out`; `README.md` gets a `## Screens` section listing the ten names with their `docs/screens/<name>.png` paths.
