# Monitoring dependency resolution

Resolved from official publisher registries on 2026-09-07. These are stable
releases, not versions inherited from either reference application's lockfile.
`web/bun.lock` and Cargo's lockfile record the installed dependency graph.

## Required toolchains

| Tool | Required version | Official evidence |
| --- | --- | --- |
| Bun | 1.4.2 | [Release](https://github.com/oven-sh/bun/releases/tag/bun-v1.4.2) |
| Rust | 1.98.1 | [Stable channel manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml) |

Bun 1.4.2 was released September 5. Rust's stable channel manifest is dated
September 3. Both were verified locally with their version commands; Rust
1.98.1 includes rustfmt and Clippy. A pre-existing `RUSTUP_TOOLCHAIN=1.98.0`
environment override can supersede the repository toolchain selection: use
`cargo +1.98.1` when that override is present.

On this macOS ARM64 installation, release stripping initially failed because
`rust-objcopy` could not load its target-directory `libLLVM.dylib`. Installing
the official component with `rustup component add llvm-tools --toolchain
1.98.1` supplied that library. `rust-objcopy --version` then succeeded and
reported LLVM 22.1.8-rust-1.98.1-stable. Keep release stripping enabled; repair
the toolchain component when this missing-library failure occurs.

Docker registry inspection found `rust:1.98.1-trixie` unavailable (`no such
manifest`), while `rust:1.98-trixie` exists for Linux AMD64 and ARM64. The
taskfmt builder therefore starts from the available minor-series image and
explicitly installs/selects Rust 1.98.1 through rustup. The agent base image
also pins its rustup toolchain argument to 1.98.1. This preserves the required
compiler patch version without depending on a nonexistent Docker tag.

Use Bun for installation and every frontend command. Scripts invoking Vite,
Vitest, or other JavaScript executables must force Bun with `bun --bun` where
needed so a Node shebang does not silently select Node.

## Frontend pins

| Package | Version | Publisher metadata |
| --- | --- | --- |
| react / react-dom | 19.2.8 | [React](https://registry.npmjs.org/react/19.2.8), [DOM](https://registry.npmjs.org/react-dom/19.2.8) |
| @tanstack/react-start | 1.168.50 | [Metadata](https://registry.npmjs.org/@tanstack/react-start/1.168.50) |
| @tanstack/react-router | 1.170.33 | [Metadata](https://registry.npmjs.org/@tanstack/react-router/1.170.33) |
| @base-ui/react | 1.8.0 | [Metadata](https://registry.npmjs.org/@base-ui/react/1.8.0) |
| shadcn (source generation CLI only) | 4.21.0 | [Metadata](https://registry.npmjs.org/shadcn/4.21.0) |
| tailwindcss / @tailwindcss/vite | 4.3.3 | [CSS](https://registry.npmjs.org/tailwindcss/4.3.3), [plugin](https://registry.npmjs.org/@tailwindcss/vite/4.3.3) |
| typescript | 7.0.2 | [Metadata](https://registry.npmjs.org/typescript/7.0.2) |
| vite | 8.2.2 | [Metadata](https://registry.npmjs.org/vite/8.2.2) |
| vitest | 5.0.0 | [Metadata](https://registry.npmjs.org/vitest/5.0.0) |
| @vitejs/plugin-react | 6.1.1 | [Metadata](https://registry.npmjs.org/@vitejs/plugin-react/6.1.1) |

Start requires Router **1.170.33 exactly**, matching the latest Router release.
Start permits Vite >=7 and React >=18. React DOM requires React ^19.2.8.
Base UI supports React 17–19. The React Vite plugin requires Vite ^8; Tailwind's
plugin supports Vite 5–8; Vitest supports Vite ^6.4, ^7, or ^8. These constraints
intersect at the listed versions. The React plugin's compiler/Babel/OXC peers
are optional. Do not introduce them unless the configuration uses them.

Node engine declarations remain upstream metadata: Start >=22.12, Vite
^20.19 or >=22.12, Vitest ^22.12 / ^24 / >=26. Runtime compatibility with Bun
is established by the repository's actual typecheck, test, and build gates,
not by interpreting those Node declarations as Bun version ranges.

Additional latest stable packages resolved for implementation: @types/react
19.2.18, @types/react-dom 19.2.7, @types/node 26.4.1,
@testing-library/react 16.3.3, @testing-library/jest-dom 7.0.1, jsdom 30.0.1,
lucide-react 1.41.0, clsx 2.1.1, tailwind-merge 3.6.0, and
class-variance-authority 0.7.1. Each was checked through its publisher's
`https://registry.npmjs.org/<package>/latest` endpoint. Only packages actually
needed belong in the installation; this research list is not an instruction to
install every optional dependency.

### Current Start conventions

The [official build-from-scratch guide](https://tanstack.com/start/latest/docs/framework/react/build-from-scratch)
uses `@tanstack/react-start/plugin/vite`, with `tanstackStart()` before
`viteReact()`, `src/router.tsx` exporting `getRouter()`, generated
`src/routeTree.gen.ts`, and `src/routes/__root.tsx` providing the document with
`HeadContent` and `Scripts`. Vite 8 provides `resolve.tsconfigPaths: true`.
The guide advises disabling `verbatimModuleSyntax` because it can leak server
bundles into client bundles. Keep strict TypeScript checking enabled separately.

Use local source-owned shadcn-style components over Base UI primitives.
`shadcn` is a CLI, not a runtime component library. Inspect the final lockfile
for unintended Radix or duplicate primitive stacks after installation.

## Rust crate resolution

Versions below came from each crate's `max_stable_version` field at
`https://crates.io/api/v1/crates/<name>`; minimum supported Rust versions came
from the matching release record. All listed minimums fit Rust 1.98.1.

| Crate | Latest stable | Minimum Rust, when checked |
| --- | --- | --- |
| [axum](https://crates.io/crates/axum/0.8.9) | 0.8.9 | 1.80 |
| [tokio](https://crates.io/crates/tokio/1.53.1) | 1.53.1 | 1.71 |
| [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | 1.56 |
| [serde_json](https://crates.io/crates/serde_json/1.0.151) | 1.0.151 | 1.71 |
| [toml](https://crates.io/crates/toml/1.1.5+spec-1.1.0) | 1.1.5+spec-1.1.0 | 1.85 |
| [tower-http](https://crates.io/crates/tower-http/0.7.1) | 0.7.1 | 1.65 |
| [tower](https://crates.io/crates/tower/0.5.3) | 0.5.3 | 1.64 |
| [ammonia](https://crates.io/crates/ammonia/4.1.4) | 4.1.4 | 1.80 |
| [pulldown-cmark](https://crates.io/crates/pulldown-cmark/0.13.4) | 0.13.4 | 1.71.1 |
| [fs2](https://crates.io/crates/fs2/0.4.3) | 0.4.3 | — |
| [uuid](https://crates.io/crates/uuid/1.26.0) | 1.26.0 | — |
| [tempfile](https://crates.io/crates/tempfile/3.27.0) | 3.27.0 | 1.63 |
| [thiserror](https://crates.io/crates/thiserror/2.0.20) | 2.0.20 | — |
| [anyhow](https://crates.io/crates/anyhow/1.0.104) | 1.0.104 | — |
| [chrono](https://crates.io/crates/chrono/0.4.45) | 0.4.45 | — |
| [clap](https://crates.io/crates/clap/4.6.6) | 4.6.6 | — |
| [regex](https://crates.io/crates/regex/1.13.1) | 1.13.1 | — |
| [sha2](https://crates.io/crates/sha2/0.11.0) | 0.11.0 | — |
| [signal-hook](https://crates.io/crates/signal-hook/0.4.4) | 0.4.4 | — |
| [walkdir](https://crates.io/crates/walkdir/2.5.0) | 2.5.0 | — |
| [http-body-util](https://crates.io/crates/http-body-util/0.1.5) | 0.1.5 | — |
| [reqwest](https://crates.io/crates/reqwest/0.13.4) | 0.13.4 | 1.85 |

This table records researched candidates, not a mandate to add unused crates.
Cargo resolution and the complete Rust checks establish compatibility of the
selected graph, including transitive dependencies.

## Installed lockfile audit

Audited `web/bun.lock` against every direct dependency's installed
`web/node_modules/<package>/package.json` on 2026-09-07. All matched. Every
required frontend version in the table above is installed, except shadcn:
its components are source-owned and its CLI is intentionally absent from the
runtime dependency graph. The ancillary installed versions are:

| Package | Installed version |
| --- | --- |
| class-variance-authority | 0.7.1 |
| clsx | 2.1.1 |
| react-markdown | 10.1.0 |
| remark-gfm | 4.0.1 |
| tailwind-merge | 3.6.0 |
| zod | 4.5.4 |
| @biomejs/biome | 2.5.12 |
| @testing-library/react | 16.3.3 |
| @testing-library/jest-dom | 7.0.1 |
| @types/react | 19.2.18 |
| @types/react-dom | 19.2.7 |
| @types/bun | 1.4.1 |
| jsdom | 30.0.1 |

These match the publisher registry's latest stable versions at audit time.
The manifest uses exact versions for the required stack; auxiliary `latest`
and compatible ranges resolve to these exact versions in `bun.lock`. Use
`bun install --frozen-lockfile` to reproduce the audited graph. Refreshing
dependencies requires repeating the compatibility and verification gates.

The primitive-stack search found exactly `@base-ui/react` 1.8.0 and its
`@base-ui/utils` 0.4.0 support package. No Radix, Headless UI, React Aria, or
second Base UI release is present. Floating UI is Base UI's positioning
dependency, not a separately adopted component stack. The application owns
its shadcn component source and `components.json` selects `base-nova`.

`harness/Cargo.lock` resolves every direct crate to the version in the Rust
table: anyhow, axum, chrono, clap, regex, serde,
serde_json, sha2, signal-hook, tempfile, toml, tokio, uuid, walkdir,
http-body-util, tower, and reqwest. Both build-time and runtime SHA-256 use sha2
0.11.0. Researched candidates fs2 and thiserror were not added. Tower HTTP is
a transitive dependency of reqwest, not a direct application dependency.
Unused ammonia and pulldown-cmark were removed from the manifest and lockfile;
Markdown rendering uses the frontend's react-markdown with raw HTML skipped.
The unused `cn` package was also removed from the frontend lockfile.
`rust-toolchain.toml` pins 1.98.1 with Clippy and rustfmt.

Application lockfiles are `web/bun.lock` and `harness/Cargo.lock`. A search
including hidden files, excluding read-only `extra/` references, generated
dependency/build directories, and `.git`, found no npm, pnpm, or Yarn
lockfiles. This audit proves dependency identity and declared compatibility;
the full application verification gates separately establish runtime behavior.

### Local CLI HTTP adapter

The project/group CLI uses reqwest **0.13.4**, with
`default-features = false` and only `blocking` and `json` enabled. The
[official release metadata](https://crates.io/api/v1/crates/reqwest/0.13.4)
declares Rust 1.85 and a compatible Tokio 1 dependency. The manifest and
`Cargo.lock` resolve 0.13.4; `cargo +1.98.1 tree -e features -i reqwest
--locked` confirmed exactly the two selected features.

This configuration provides HTTP/1 and typed JSON without enabling reqwest's
default TLS, HTTP/2, charset, or system-proxy features. It intentionally cannot
serve as a general HTTPS client. The adapter calls `no_proxy()` and disables
redirects with `redirect::Policy::none()`; both are documented on the
[official blocking client builder](https://docs.rs/reqwest/0.13.4/reqwest/blocking/struct.ClientBuilder.html).
Together with loopback URL validation, these keep CLI requests directed to the
local monitor. The [blocking API](https://docs.rs/reqwest/0.13.4/reqwest/blocking/index.html)
must run outside an asynchronous runtime; synchronous CLI dispatch owns these
requests. API failures are decoded from their typed JSON bodies.

The [sha2 0.11 API](https://docs.rs/sha2/0.11.0/sha2/) retains the
`Sha256::new`, `update`, and `finalize` interface used by the existing harness.
The [signal-hook 0.4 flag API](https://docs.rs/signal-hook/0.4.4/signal_hook/flag/fn.register.html)
retains `register(c_int, Arc<AtomicBool>) -> Result<SigId, Error>`. Existing
digest vectors, fingerprint tests, and supervisor tests remain the behavioral
checks for these upgrades.
