# TASK-102 — fixed decisions (verbatim)

Planner-owned. Numbering is global across the pgtui task series. Implement; do not reopen. D-001..D-005 (workspace, module layout, test placement, lint, justfile) are unchanged from TASK-101 and remain in force.

## App state machine

- **D-010 Screen enum** (exact):

  ```rust
  pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }
  ```

  `App` fields (all pub for tests): `screen: Screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>`, `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`.
  `CreateForm` (this task, exact): `pub struct CreateForm { pub name: String, pub host: String, pub port: String, pub dbname: String, pub username: String, pub password: String, pub focus: Field }`, `pub enum Field { Name, Host, Port, Database, User, Password }` (order = D-032), `impl CreateForm { pub fn blank() -> Self; pub fn validate(&self) -> Result<NewConnection, String>; }`.
- **D-011 Elm-style core.** `App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure (no IO, no async). `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs IO. Tests drive `App` with `Msg` values directly; snapshots never need a database. `Msg`/`Effect`/`QueryKind` enums exactly as in TASK-101 (`Msg::Saved(Result<SavedConnection, StoreError>)`, `Effect::SaveConnection(NewConnection)`, `Effect::LoadConnections`).
- **D-012 Runtime loop.** `main.rs`: parse CLI → open store (fail → exit 2) → enter raw mode/alternate screen → `App::update(Msg::Connections(store.list()))` → loop { draw; read key (blocking); effects = update(Key); for each effect: execute → feed reply Msg → collect further effects } until `Effect::Quit`. DB calls block the loop.
- **D-013 Status line.** Bottom row, height 1, full width. `Status::Help` shows the screen's key hint (D-060). `Info`/`Error` replace it until the next key press, then revert to `Help`. Error text is `error: <message>`; message is the first line of the underlying error, truncated to width. No modal dialogs.

## Storage (frozen)

- **D-021 Schema** (executed on every open, idempotent): table `connections(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, host TEXT NOT NULL, port INTEGER NOT NULL, dbname TEXT NOT NULL, username TEXT NOT NULL, password TEXT NOT NULL, created_at TEXT NOT NULL)`. Password plaintext; keyring out of scope.
- **D-022 Store API** (exact; protected from this task on):

  ```rust
  pub struct NewConnection { pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String }
  pub struct SavedConnection { pub id: i64, pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String, pub created_at: String }
  pub enum StoreError { Open(String), DuplicateName(String), Sql(String) }
  impl ConnectionStore {
      pub async fn open(path: &std::path::Path) -> Result<Self, StoreError>;
      pub async fn list(&self) -> Result<Vec<SavedConnection>, StoreError>;   // ORDER BY name ASC
      pub async fn insert(&self, new: NewConnection) -> Result<SavedConnection, StoreError>;
  }
  impl SavedConnection { pub fn display_dsn(&self) -> String; }  // "user@host:port/dbname", never the password
  ```

## Key bindings

- **D-032 CreateConnection.** Fields in order: Name, Host, Port, Database, User, Password. `Tab` next field, `BackTab` previous (wrap). Printable chars append to the focused field; `Backspace` pops. Port accepts digits only (other chars ignored). `Enter` validates: all fields non-empty except Password (may be empty), Port parses `u16` ≥ 1; on failure `Status::Error` with the first failing field (`error: <field> is required` / `error: port must be 1-65535`; field labels as displayed: `Name`, `Host`, `Port`, `Database`, `User`), stay on form. On success → `Effect::SaveConnection`; `Msg::Saved(Ok)` → reload list (`Effect::LoadConnections`), `Screen::ConnectionList`, cursor on the new row; `Msg::Saved(Err(DuplicateName))` → `error: name already exists`, stay on form. `Esc` → `Screen::ConnectionList`, form discarded.

## Errors

- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## Layout (deterministic; snapshots depend on it)

- **D-060 Frame.** Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.
  - CreateConnection: block title ` New connection `; six lines `  <Label>: <value>` (labels `Name`, `Host`, `Port`, `Database`, `User`, `Password`), focused line prefixed `> ` instead of two spaces, Password shown as `*` per char. Help: `Tab next  Enter save  Esc cancel`.
  - ConnectionList (unchanged from TASK-101): title ` pgtui - connections `, rows `<name>  <display_dsn>`, `highlight_symbol` `> `, help `j/k move  Enter connect  n new  q quit`.
- **D-061** `ui::draw(frame: &mut Frame, app: &App)`; tests render with `TestBackend::new(100, 30)`.

## Snapshots

- **D-071 Snapshot policy.** Text via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `"<name>__svg"`. Names for this task: `screen__create_form_blank`, `screen__create_form_filled` (all fields filled, Password `secret` focused → `******`), `screen__create_form_saved_list` (list after save with the new row selected). All `.snap` files are planner-shipped and protected. Gate runs with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; `*.snap.new` anywhere fails the gate.
