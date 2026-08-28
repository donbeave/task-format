# Example Rust TUI app — testing stack research

Date: 2026-08-28. Purpose: fixture app (ratatui + crossterm + tokio + tokio-postgres + Turso/SQLite for local connection store) whose `verify.config` must prove: unit tests, testcontainers integration against real PostgreSQL, text snapshots of every screen, visual PNG/SVG renders of the same screens. Agent runs headed inside a persistent Docker container (see `container-harness.md`), so everything below must work with no display, no X11, Docker reachable only via a mounted socket.

Legend: **[VERIFIED]** = read from crates.io API / docs.rs / repo source today. **[UNVERIFIED]** = not executed / not read in source; must be confirmed when the fixture is built.

## 1. Versions (crates.io, 2026-08-28) [VERIFIED]

| Crate | Version | Updated | Note |
|---|---|---|---|
| `testcontainers` | 0.28.0 | 2026-08-06 | bollard 0.21 under the hood (0.28 breaking bump) |
| `testcontainers-modules` | 0.15.0 | 2026-02-21 | feature `postgres` |
| `insta` / `cargo-insta` | 1.48.0 | 2026-06-11 | |
| `ratatui` | 0.30.2 | 2026-06-19 | split into `ratatui-core` 0.1.2 + widgets; `TestBackend` still `ratatui::backend::TestBackend` |
| `ratatui-macros` | 0.7.2 | 2026-06-19 | not required for testing |
| `crossterm` | 0.29.0 | 2025-04-05 | |
| `tokio-postgres` | 0.7.18 | 2026-06-12 | |
| `turso` | 0.7.2 | 2026-08-21 | `Builder::new_local(":memory:")` for tests |
| `libsql` | 0.9.30 | 2026-06-02 | alternative to `turso` if API gaps |
| `term-transcript` | 0.5.0 | 2026-07-18 | ANSI -> SVG with colors; depends on `styled-str` 0.5 |
| `resvg` | 0.48.1 | 2026-08-02 | pairs with `usvg ^0.48.1`, `tiny-skia ^0.12` |
| `image` | 0.25.10 | 2026-03-10 | PNG decode for verifier |
| `image_hasher` | 3.1.1 | 2026-02-21 | perceptual hash |
| `cargo-nextest` | 0.9.143 | 2026-08-04 | |
| `rstest` | 0.26.1 | 2025-07-27 | |
| `pretty_assertions` | 1.4.1 | 2024-09-15 | |
| `ratatui-testlib` | 0.1.0 | 2025-12-01 | PTY+vt100 harness, `snapshot-insta` feature; young, single release |
| `ansi-to-svg` | 0.1.1 | 2024 | 0% documented; skip |
| `ansi-to-html` | 0.2.4 | 2026-08-15 | HTML only; no rasterizer without browser; skip |
| `ratatui-image` | 11.0.6 | | renders images *into* a TUI (sixel/kitty); wrong direction; skip |
| `tui-term` | 0.3.4 | | PTY widget; not a test tool; skip |
| `termsvg` | n/a | | not on crates.io (Go tool); skip |

## 2. Integration tests: testcontainers + PostgreSQL

### 2.1 API (0.28) [VERIFIED from docs.rs + repo source]

```toml
[dev-dependencies]
testcontainers = "0.28"
testcontainers-modules = { version = "0.15", features = ["postgres"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tokio-postgres = "0.7"
```

```rust
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn lists_tables() -> anyhow::Result<()> {
    let node = Postgres::default()
        .with_tag("16-alpine")                 // ImageExt; module default TAG is "11-alpine" [VERIFIED src]
        .with_db_name("app")
        .with_user("app")
        .with_password("secret")
        .with_init_sql(include_str!("fixtures/schema.sql").to_string()) // impl Into<CopyDataSource>
        .start()
        .await?;                               // ContainerAsync<Postgres>
    let host = node.get_host().await?;         // url::Host; "localhost" or bridge gateway when in a container
    let port = node.get_host_port_ipv4(5432).await?;
    let url = format!("postgres://app:secret@{host}:{port}/app");
    let (client, conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls).await?;
    tokio::spawn(conn);
    let rows = client.query("SELECT tablename FROM pg_tables WHERE schemaname='public'", &[]).await?;
    assert!(!rows.is_empty());
    Ok(())
}   // `node` dropped here -> container removed (Drop). Keep it alive for the whole test.
```

Facts:
- `Postgres` module: env `POSTGRES_DB/USER/PASSWORD` default `postgres`; `fsync=off` by default (`with_fsync_enabled()` to revert); wait strategy = `"database system is ready to accept connections"` on stderr **and** stdout; builders `with_host_auth`, `with_db_name`, `with_user`, `with_password`, `with_init_sql`. [VERIFIED src]
- `ContainerAsync`: `get_host() -> Result<Host>`, `get_host_port_ipv4(impl Into<ContainerPort>) -> Result<u16>`, `exec`, `stdout(follow)`, `stderr(follow)`, `stop`, `rm(self)`, `pause`, `id()`. All async. [VERIFIED docs.rs]
- Containers are removed on `Drop`. Override with `TESTCONTAINERS_COMMAND=keep` (values `keep`|`remove`). [VERIFIED src]
- Changelog 0.25→0.28: reusable containers (0.26, needs a name; stopped-reuse 0.26.3), docker-compose support (0.26), `DOCKER_DEFAULT_PLATFORM` (0.25.1), "allow disabling default wait behaviour" (0.27.1), bollard 0.21 (0.28, marked breaking). No `AsyncRunner`/port API change. [VERIFIED CHANGELOG.md]
- Pull is automatic; first run downloads `postgres:16-alpine` — pre-pull in the harness image build to keep FOCUSED_CMDS time bounded.

### 2.2 Ryuk — Rust has none [VERIFIED]

`gh api search/code?q=ryuk+repo:testcontainers/testcontainers-rs` → 0 hits. testcontainers-rs does **not** run the Ryuk reaper. Consequences:
- `TESTCONTAINERS_RYUK_DISABLED`, `TESTCONTAINERS_HOST_OVERRIDE`, `tc.host` semantics from Java/Go docs do not apply (only `tc.host` exists as a *properties* key for docker host). Do not put `TESTCONTAINERS_RYUK_DISABLED=true` in verify docs as if it does anything (harmless but misleading).
- Cleanup is `Drop` only. If the test binary is SIGKILLed (nextest `terminate-after`, OOM), containers leak. Mitigation: verifier pre-step `docker ps -aq --filter label=org.testcontainers.managed-by=testcontainers` [UNVERIFIED label name — inspect a started container and pin the exact label] or simply `docker ps -a --filter ancestor=postgres:16-alpine` and remove stale ones.

### 2.3 Env vars / resolution actually implemented [VERIFIED `core/env/config.rs`, `docs/features/configuration.md`]

Env: `DOCKER_HOST`, `DOCKER_TLS_VERIFY`, `DOCKER_CERT_PATH`, `TESTCONTAINERS_COMMAND`, `DOCKER_DEFAULT_PLATFORM`, `DOCKER_AUTH_CONFIG`, `DOCKER_CONFIG`.
Properties (`~/.testcontainers.properties`, feature `properties-config`): `tc.host`, `docker.host`, `docker.tls.verify`, `docker.cert.path`.
Docker host order: `tc.host` → `DOCKER_HOST` → `docker.host` → `/var/run/docker.sock` → rootless sockets (`XDG_RUNTIME_DIR`, `$HOME/.docker/run/docker.sock`) → socket with schema.

### 2.4 Running inside the agent container (socket mount, "wormhole") [VERIFIED `core/client.rs`]

```rust
async fn is_in_container() -> bool { tokio::fs::metadata("/.dockerenv").await.is_ok() }
// docker_hostname(): scheme tcp/http/https -> URL host;
// unix/npipe -> if is_in_container() { gateway of docker network "bridge" (e.g. 172.17.0.1) } else { "localhost" }
```

So with the socket mounted, `get_host()` returns the **bridge gateway IP**, and published ports on that IP are reachable from the agent container **only if** the agent container is on the default `bridge` network (or the host firewall allows container→gateway traffic). Setup for the harness container:

```bash
docker run -d --name agent-<task> \
  -v /var/run/docker.sock:/var/run/docker.sock \
  --group-add "$(stat -c %g /var/run/docker.sock)" \   # Linux; macOS Docker Desktop socket gid is root/0 -> run as root or use --group-add 0
  -e DOCKER_HOST=unix:///var/run/docker.sock \
  -v "$PWD:$PWD" -w "$PWD" \                            # same path inside+outside; needed only if tests bind-mount host paths
  agent-image
```

Pitfalls:
- `/.dockerenv` must exist in the agent container (Docker creates it; Podman/containerd may not → `get_host()` returns `localhost`, which is wrong with a mounted socket). Fallback: set `DOCKER_HOST=tcp://host.docker.internal:2375`-style TCP so the URL host wins [UNVERIFIED that Docker Desktop exposes TCP by default — it does not; needs `dockerd -H tcp://...` or a socat sidecar].
- Docker Desktop on macOS: the bridge gateway `172.17.0.1` is **not** reachable from a container in many configurations; sibling containers' published ports are reachable via `host.docker.internal`. Rust has no `TESTCONTAINERS_HOST_OVERRIDE`; workaround = attach the agent container and test containers to a user-defined network and connect via container IP (`get_bridge_ip_address()` + internal port 5432, no port mapping needed), or run the harness on Linux. Must be measured on the operator's actual host before choosing. [UNVERIFIED on macOS]
- True DinD (`docker:dind` sidecar, `DOCKER_HOST=tcp://docker:2376`): works (scheme tcp → host `docker`), ports published on the dind container → reachable by name. Testcontainers upstream calls DinD "instrument of last resort"; socket mount preferred.
- Leaks: see 2.2. Set `TESTCONTAINERS_COMMAND=remove` explicitly in `.cargo/config.toml [env]` so an operator's `keep` in their shell does not survive into the verifier.
- `with_init_sql` copies the file into `/docker-entrypoint-initdb.d/`; no host bind mounts → the "same path" rule above is not needed for this fixture.

### 2.5 Parallelism: nextest [VERIFIED nexte.st]

Each `#[tokio::test]` spawns its own container (~1–2 s on cached image). nextest runs one process per test; cap concurrency:

```toml
# .config/nextest.toml
[test-groups]
docker = { max-threads = 2 }

[[profile.default.overrides]]
filter = 'binary(integration_pg)'      # tests/integration_pg.rs
test-group = 'docker'
slow-timeout = { period = "60s", terminate-after = 2 }
retries = 0

[profile.ci]                          # select with --profile ci or NEXTEST_PROFILE=ci
fail-fast = false
```

Or share one container per test binary via `tokio::sync::OnceCell<ContainerAsync<Postgres>>` static — faster but tests must not mutate shared tables (use per-test schemas). Recommended for fixture: one container per test file, per-test `CREATE SCHEMA`.

## 3. Text snapshots: ratatui TestBackend + insta

### 3.1 Rendering deterministically [VERIFIED docs.rs ratatui 0.30.2 / ratatui.rs recipe]

```rust
use ratatui::{backend::TestBackend, Terminal};

fn render(app: &App, w: u16, h: u16) -> TestBackend {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| app.render(f, f.area())).unwrap();
    term.into_inner()   // or keep `term` and use term.backend()
}
```

- `TestBackend::new(width, height)`, `buffer() -> &Buffer`, `assert_buffer(&Buffer)`, `assert_buffer_lines(...)`, `resize(w,h)`.
- `Display for TestBackend` prints `buffer_view` = **text only, no styles**. The official recipe `assert_snapshot!(terminal.backend())` therefore cannot detect color regressions ("Asserting with color is not supported as of now" — ratatui.rs).
- `Debug for Buffer` prints `area`, `content` lines, **and** a `styles:` list with one entry per style *change* (`x, y, fg, bg, modifier`). [VERIFIED ratatui-core/src/buffer/buffer.rs]

```text
Buffer {
    area: Rect { x: 0, y: 0, width: 12, height: 2 },
    content: [
        "Hello World!",
        "G'day World!",
    ],
    styles: [
        x: 0, y: 0, fg: Reset, bg: Reset, modifier: NONE,
        x: 0, y: 1, fg: Green, bg: Yellow, modifier: BOLD,
    ]
}
```

Use **two snapshots per screen**: `assert_snapshot!(name, backend)` (readable text, for humans and ACs) and `assert_debug_snapshot!(name_styles, backend.buffer())` (styles). The debug one is what makes "column sorted asc/desc" header highlight or the selected sidebar row provable.

Determinism rules:
- Fixed size per screen, e.g. 100x30, set in one `const SNAP_SIZE: (u16,u16)`; never read the real terminal. Widget tests must not depend on `crossterm::terminal::size()`.
- No wall-clock, no random IDs in rendered text; if unavoidable, `insta::Settings::add_filter(r"\d{4}-\d{2}-\d{2}T[^ ]+", "[TS]")`.
- App state for screens is built by pure constructors (fixture rows), not from a live DB. Grid content for `SELECT * FROM t LIMIT 500` comes from a fixture `Vec<Row>` — the DB path is covered by integration tests, not snapshots.
- Unicode: box-drawing chars are fine; avoid emoji (width ambiguity across `unicode-width` versions).
- Do not snapshot inside `#[tokio::test]` multi-thread runtimes with shared static state.

Screens to snapshot (each text + styles): `connection_list`, `connection_create`, `connected_empty` (sidebar tables, empty body), `table_select_500`, `table_sorted_asc`, `table_sorted_desc`, `sql_custom_empty`, `sql_custom_results`. Drive state via the app's own event handler (`app.handle(KeyCode::Down)`) so the snapshot exercises real transitions, not hand-built state.

### 3.2 insta mechanics [VERIFIED insta.rs docs + cargo-insta cli.rs]

- `INSTA_UPDATE`: `auto` (default: `no` on CI, else `new`), `always`, `unseen`, `new` (writes `.snap.new`), `no` (just run). CI detection = `CI` env var set [UNVERIFIED exact detection list]. In the verifier set `INSTA_UPDATE=no CI=1` explicitly.
- `INSTA_FORCE_PASS=1` makes assertions pass (never in verify). `INSTA_WORKSPACE_ROOT` if cargo metadata lookup fails.
- `cargo insta test --check` — "Instructs the test command to just assert" (fail on mismatch, no writes). `--accept`, `--accept-unseen`, `--review`, `--unreferenced=reject|delete|warn|ignore|auto`, `--test-runner nextest`, `--test <name>`, `-p`, `--workspace`. Exit 1 if pending snapshots and no `--review/--accept`.
- Config `.config/insta.yaml`: `test.runner: nextest`, `behavior.force_update`, `review.include_ignored`.
- Snapshot paths: by default `snapshots/` next to the test file, name `<binary>__<module>__<test>.snap`; `Settings::set_snapshot_path("snapshots/text")`, `set_prepend_module_to_snapshot(false)`, `set_snapshot_suffix`, `with_settings!({...}, { ... })`.
- Stale-snapshot guard: `cargo insta test --check --unreferenced=reject` fails if a `.snap` has no matching assertion (catches deleted screens with dangling files).

## 4. Visual (PNG/SVG) renders

Requirement: same 8 screens as images, colors preserved, no display, deterministic. Options evaluated:

| Option | Runs headless in container | Deterministic | Colors | Verdict |
|---|---|---|---|---|
| **A. Buffer → own SVG writer → resvg → PNG** | yes (pure Rust) | yes (bundled font, fixed cell grid) | exact from `Cell.fg/bg/modifier` | **Recommended** |
| B. Buffer → ANSI string → `term-transcript` `Template::render` SVG → resvg PNG | yes | yes-ish (template font stack; embed font) | yes (`styled-str` parses SGR) | Good alternative; less code, but template geometry not cell-exact [UNVERIFIED wrap/width behavior for 100-col buffers] |
| C. `vhs` (charm) driving real binary in `ghcr.io/charmbracelet/vhs` | yes (ttyd+chromium+ffmpeg inside image) | timing-based (`Sleep`), font antialiasing varies | yes | Good for a demo GIF, not for pass/fail; needs the real DB → slower, flaky |
| D. `ratatui-testlib` (PTY + vt100) | yes | text-level only; no rasterizer | cell attrs available | Could replace TestBackend for end-to-end key-driving; no image output; single 0.1.0 release |
| E. `ansi-to-svg` / `ansi-to-html` | yes | — | yes | undocumented / no rasterizer; skip |

### 4.1 Option A pipeline (recommended)

`tests/visual.rs` (or `src/testing/render.rs` behind `cfg(test)`/feature `visual`):

1. Render each screen with `TestBackend` exactly as in §3 (same size, same state) → `&Buffer`.
2. Convert `Buffer` → SVG string. Fixed metrics: cell `CW=9`, `CH=18` px at font-size 15; canvas `width = CW*cols`, `height = CH*rows`. Emit: one background `<rect>` per run of equal `bg`, one `<text>` per run of equal `(fg, modifier)` with `xml:space="preserve"`, `font-weight=bold` for `Modifier::BOLD`, `font-style=italic`, `text-decoration=underline`, `fill-opacity=.5` for `DIM`, swap fg/bg for `REVERSED`. Map `Color::Reset` → theme default (`#1e1e2e` bg, `#cdd6f4` fg), 16 ANSI colors → fixed hex table, `Color::Rgb`/`Indexed` → exact/256-table. Width-2 graphemes: skip the following cell (mirror Buffer Debug logic).
3. Write `target/visual/<screen>.svg` **and** `insta::assert_snapshot!("<screen>.svg", svg)` — the SVG is text, so it doubles as a third snapshot (styling as geometry) and diffs cleanly.
4. Rasterize with resvg [VERIFIED resvg examples/minimal.rs]:

```rust
let mut opt = usvg::Options::default();
opt.fontdb_mut().load_font_data(include_bytes!("../assets/DejaVuSansMono.ttf").to_vec()); // never load_system_fonts(): container has none
opt.font_family = "DejaVu Sans Mono".to_string();
let tree = usvg::Tree::from_str(&svg, &opt)?;
let size = tree.size().to_int_size();
let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
pixmap.save_png(format!("target/visual/{name}.png"))?;
```

`Cargo.toml` dev-deps: `resvg = "0.48"`, `usvg = "0.48"`, `tiny-skia = "0.12"` (resvg re-exports both; use `resvg::usvg`, `resvg::tiny_skia`). Bundle a font with a permissive license (DejaVu Sans Mono: Bitstream Vera license; ~340 KB) under `assets/`. [UNVERIFIED: `usvg::Options::font_family` field name in 0.48 — check; it existed as `font_family: String` in 0.4x].

5. Optional exactness gate: compute `image_hasher` (`HasherConfig::new().to_hasher().hash_image(&img)`; `hash.dist(&other)`) against committed `tests/visual/expected/<screen>.phash` (base64 via `to_base64()`), assert `dist <= 2`. Because everything is pure Rust with a bundled font, byte-identical PNGs are expected across Linux containers; a pixel-exact `assert_eq!(bytes)` is also viable but brittle across resvg upgrades — phash threshold is the safer gate; PNG byte hash can be an EXTRA_CHECK that warns.

6. For humans: also write `target/visual/index.html` listing all PNGs (agent copies into `progress.md` evidence section as paths; operator opens via `docker cp`).

### 4.2 Option B specifics (term-transcript) [VERIFIED docs.rs 0.5.0]

- `Transcript::new()`, `add_interaction(impl Into<UserInput>, StyledString)`, `Interaction::new(input, StyledString)`; `StyledString::from_ansi(&str) -> Result<_, AnsiError>` (styled-str 0.5; SGR only, other CSI dropped).
- `term_transcript::svg::{Template, TemplateOptions}`; `TemplateOptions{ width, palette, font_family, ...}.validated()?` → `Template::new(opts).render(&transcript, &mut writer)`; `Template::pure_svg(...)` for renderers without `foreignObject` (needed for resvg — use `pure_svg`). `with_font_embedder` can embed the font into the SVG (feature `font-subset`).
- Not a shell, no PTY needed: serialize `Buffer` cells to an ANSI string (`anstyle` Style → `{style}` prefix per run, `\n` per row) and parse it back. Reverse-video and 256/RGB colors are supported by anstyle. [UNVERIFIED: `pure_svg` output rendering fidelity in resvg 0.48 — try before committing].

### 4.3 Why not vhs for verification

Needs the real binary + real DB inside the tape run, wall-clock `Sleep`s, Chromium font rendering → non-deterministic pixels; fine as an operator-facing demo (`Output demo.gif`), and it does run headless via `docker run --rm -v $PWD:/vhs ghcr.io/charmbracelet/vhs demo.tape` — but that is DinD-from-the-agent again. Keep out of FOCUSED/REGRESSION.

## 5. Test layout

```
example-app/
  Cargo.toml
  .cargo/config.toml            # [env] INSTA_UPDATE="no" not here (agent needs `new` locally); put TESTCONTAINERS_COMMAND="remove"
  .config/nextest.toml          # test-groups docker, timeouts
  .config/insta.yaml            # test.runner: nextest
  assets/DejaVuSansMono.ttf
  src/                          # unit tests in-module: #[cfg(test)] mod tests (sort comparator, SQL builder `SELECT * FROM "t" LIMIT 500`, connection-store CRUD on turso ":memory:")
  tests/
    common/mod.rs               # fixtures: sample rows, App builders, render(), rstest fixtures
    integration_pg.rs           # testcontainers; rstest #[fixture] async pg() -> (ContainerAsync<Postgres>, Client)
    snapshots.rs                # 8 screens x (text + styles) insta
    snapshots/                  # *.snap committed (insta default dir next to tests/)
    visual.rs                   # 8 screens -> target/visual/*.svg|*.png + svg .snap + phash gate
    visual/expected/*.phash     # committed hashes (or *.png goldens if pixel-exact chosen)
    fixtures/schema.sql         # with_init_sql
```

- `rstest` 0.26: `#[rstest] #[case(SortDir::Asc)] #[case(SortDir::Desc)]` for sort snapshots; `#[fixture] async fn pg()` + `#[tokio::test]` attribute on the rstest fn (rstest supports async fixtures with `#[future]`). [UNVERIFIED exact `#[future]` syntax in 0.26; documented since 0.12]
- `pretty_assertions::assert_eq` in unit tests for row/grid structs; not needed for insta (has its own diff).
- Turso store tests: `Builder::new_local(":memory:").build().await`, `db.connect()`, `conn.execute(sql, ()).await`, `conn.prepare(..).await` → `stmt.query([..]).await` → `rows.next().await` → `row.get_value(i)`. [VERIFIED docs.rs turso 0.7.2]. Keep `turso` behind a `ConnectionStore` trait so the TUI snapshots use an in-memory fake.

## 6. verify.config commands

```bash
# Focused (one per AC)
FOCUSED_CMDS=(
  "cargo nextest run --lib"                                                 # AC unit
  "cargo nextest run --test integration_pg"                                 # AC pg via testcontainers
  "env CI=1 INSTA_UPDATE=no cargo insta test --check --unreferenced=reject --test-runner nextest --test snapshots"   # AC text snapshots
  "env CI=1 INSTA_UPDATE=no cargo insta test --check --test-runner nextest --test visual"                             # AC visual (writes target/visual/*.png, asserts svg snaps + phash)
)
REGRESSION_CMDS=("env CI=1 INSTA_UPDATE=no cargo insta test --check --unreferenced=reject --test-runner nextest --workspace")
LINT_CMDS=("cargo fmt --all -- --check" "cargo clippy --all-targets --all-features -- -D warnings")
FORBIDDEN_PATTERNS=('#\[ignore\]|tests src' 'INSTA_FORCE_PASS|tests src .cargo .config' 'load_system_fonts|tests src' '\.snap\.new$|tests')
REQUIRED_PATHS=(
  "tests/snapshots/snapshots__connection_list.snap" "tests/snapshots/snapshots__connection_list_styles.snap"
  "tests/snapshots/snapshots__table_sorted_asc.snap" "tests/snapshots/snapshots__table_sorted_desc.snap"
  "tests/snapshots/visual__connection_list.svg.snap"
)
EXTRA_CHECKS=(visual_pngs no_snap_new no_leaked_containers)

check_visual_pngs() {
  local d="$VERIFY_ROOT/target/visual" fail=0
  for s in connection_list connection_create connected_empty table_select_500 table_sorted_asc table_sorted_desc sql_custom_empty sql_custom_results; do
    local f="$d/$s.png"
    [[ -f "$f" ]] || { echo "missing $f"; fail=1; continue; }
    [[ $(stat -c %s "$f" 2>/dev/null || stat -f %z "$f") -gt 4096 ]] || { echo "too small $f"; fail=1; }
    file "$f" | grep -qE 'PNG image data, [0-9]+ x [0-9]+' || { echo "not png $f"; fail=1; }
    # dimensions: 100x30 cells * 9x18 px = 900x540
    file "$f" | grep -q '900 x 540' || { echo "bad dims $f: $(file "$f")"; fail=1; }
    # non-trivial: >= 2 distinct colours; ImageMagick if present, else python3+PIL, else skip with note
    if command -v identify >/dev/null; then
      [[ $(identify -format %k "$f") -ge 2 ]] || { echo "flat image $f"; fail=1; }
    fi
  done
  return $fail
}
check_no_snap_new() { ! find "$VERIFY_ROOT/tests" -name '*.snap.new' | grep -q .; }
check_no_leaked_containers() { [[ -z "$(docker ps -q --filter ancestor=postgres:16-alpine)" ]]; }
```

Notes:
- `cargo insta test --check` returns non-zero on mismatch **and** on new/pending snapshots; `--unreferenced=reject` fails on orphan `.snap` files. Both required so the agent cannot "pass" by deleting an assertion.
- `cargo insta test` with `--test-runner nextest` shells out to `cargo nextest run`; if nextest is absent it errors unless `--test-runner-fallback`. Harness image must install `cargo-nextest` and `cargo-insta` (`cargo install cargo-nextest --locked`, `cargo install cargo-insta --locked`) — bake into image, not per-run.
- Snapshot names: insta prefixes with the test binary and module: `tests/snapshots.rs` → `tests/snapshots/snapshots__<name>.snap`. Verify REQUIRED_PATHS after first `--accept` run and pin exact names.
- Visual test names PNGs deterministically; `target/` is gitignored → PNGs are artifacts, SVG `.snap` + `.phash` are committed evidence.
- Perceptual-hash gate lives in Rust (`image_hasher` 3.1, `image` 0.25) so verify.sh needs no Python. `identify`/`file` only for the cheap shell-level sanity check; `file` is in base images, ImageMagick optional.
- FORBIDDEN `load_system_fonts`: guarantees the bundled font path (determinism).

## 7. Pitfalls checklist

1. Snapshot flapping from terminal size: never call `crossterm::terminal::size()` in render code paths that tests exercise; size is injected.
2. `Buffer` Debug lists style *changes* — a widget refactor that splits one span into two with identical style will not change the snapshot (good), but a change in `Reset` vs explicit color will (intended).
3. ratatui 0.30 renamed `Frame::size()` → `Frame::area()`; widget trait moved to `ratatui-core`/`ratatui-widgets` — imports via `ratatui::` facade still work.
4. tokio-postgres connection future must be `tokio::spawn`ed; dropping the container while the `Client` lives causes hangs on next query → keep `node` in the same scope as `client`, drop client first.
5. Postgres readiness: the entrypoint restarts the server once after init scripts; the module waits for the "ready" message on both streams — with `with_init_sql` the first "ready" is the temporary server. Upstream handles this with two WaitFor entries; if flaky, add `Postgres::default().with_init_sql(...)` + retry connect loop (5 x 200 ms). [UNVERIFIED flakiness in 0.15]
6. macOS Docker Desktop host: bridge gateway not routable from agent container — decide network strategy on the real operator host (§2.4).
7. Containers leak on SIGKILL (no Ryuk). nextest `terminate-after` sends SIGKILL after SIGTERM grace; Drop runs on SIGTERM only if the test process handles it — it does not. `check_no_leaked_containers` catches it; operator cleans with `docker rm -f`.
8. `INSTA_UPDATE=new` writes `.snap.new` next to snapshots when the agent runs tests locally; those must be accepted (`cargo insta accept`) before verify; `check_no_snap_new` enforces.
9. `cargo insta test` builds with `--all-targets`? No — pass `--test <name>` or `--workspace`; doctests are run unless `--disable-nextest-doctest`.
10. resvg text: `xml:space="preserve"` mandatory or leading spaces collapse; use `<text>` per row with `x` per run, not `<tspan>` with dx (kerning drift).
11. Font metrics: DejaVu Sans Mono advance at 15 px ≈ 9.03 px; either set `textLength` on each `<text>` run to `CW*len` with `lengthAdjust="spacingAndGlyphs"` or pick font-size where advance is integral (e.g. 14.94). Confirm by rendering a full-width row and checking no overflow. [UNVERIFIED exact metric]
12. `image_hasher` default alg is Gradient/dHash 8x8; TUI screens with mostly-text differ subtly — use `HashAlg::DoubleGradient` with `hash_size(16,16)` for more sensitivity, tune threshold on the 8 real screens (expect dist between *different* screens ≥ 10, same screen = 0). [UNVERIFIED thresholds]

## 8. Sources

- crates.io API (versions, dates) — queried 2026-08-28.
- https://docs.rs/testcontainers/0.28.0 ; https://docs.rs/testcontainers/latest/testcontainers/core/struct.ContainerAsync.html
- https://github.com/testcontainers/testcontainers-rs — `testcontainers/src/core/client.rs` (`is_in_container`, `docker_hostname`), `testcontainers/src/core/env/config.rs`, `CHANGELOG.md`, `docs/features/configuration.md`
- https://github.com/testcontainers/testcontainers-rs-modules-community — `src/postgres/mod.rs`
- https://java.testcontainers.org/supported_docker_environment/continuous_integration/dind_patterns/ (wormhole pattern; Java-only `TESTCONTAINERS_HOST_OVERRIDE`)
- https://insta.rs/docs/advanced/ ; https://insta.rs/docs/cli/ ; https://insta.rs/docs/settings/ ; https://github.com/mitsuhiko/insta/blob/master/cargo-insta/src/cli.rs
- https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html ; https://github.com/ratatui/ratatui/blob/main/ratatui-core/src/buffer/buffer.rs ; https://ratatui.rs/recipes/testing/snapshots/
- https://docs.rs/term-transcript/0.5.0 ; https://docs.rs/styled-str/0.5.0
- https://github.com/linebender/resvg/blob/main/crates/resvg/examples/minimal.rs
- https://docs.rs/image_hasher/3.1.1 ; https://docs.rs/ratatui-testlib/0.1.0 ; https://docs.rs/turso/0.7.2
- https://nexte.st/docs/configuration/test-groups/ ; https://nexte.st/docs/configuration/per-test-overrides/
- https://github.com/charmbracelet/vhs
