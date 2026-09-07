# Task monitor frontend

Requires Bun **1.4.2**. Bun owns installation and every frontend command.

From this directory:

1. `bun install --frozen-lockfile`
2. From the repository root, start `harness/target/debug/task-monitor --projects-root projects --origin http://127.0.0.1:5173`. It listens on `127.0.0.1:3001` (see `../docs/monitoring.md`).
3. `bun run dev`, then open `http://127.0.0.1:5173`.

Vite uses a strict port and proxies `/api` to the local Rust backend. Execution
uses validated backend IDs and the required `X-Task-Monitor: 1` header. No browser
command or filesystem path enters the execution API. Data refreshes every three
seconds. Failed refreshes retain clearly marked prior data and disable execution.
The supplied `projects/` root contains project/group/task directories. New
directories appear on refresh; browser routes such as `/tasks/my` retain their
names independently of the filesystem root. Existing run roots are bound to
their catalog path; follow the migration section in `../docs/monitoring.md`
before relocating a catalog with durable run evidence.

Production: `bun run build`, then `bun run preview` serves the generated Start SPA
at the same local port, including nested-route fallback and API proxy. Stop the dev
server before preview. For another static server, serve `dist/client`, rewrite
non-asset routes to its generated `_shell.html`, and reverse-proxy `/api` to the
Rust backend; explicitly configure the corresponding allowed origin.

Verification: `bun run typecheck`, `bun run lint`, `bun run test`, `bun run build`.
`bun run format` formats source. Generated output is ignored; `bun.lock` is committed.

Exact runtime pins: React/React DOM 19.2.8; TanStack Start 1.168.50; TanStack
Router 1.170.33; Base UI 1.8.0; react-markdown 10.1.0; remark-gfm 4.0.1;
Zod 4.5.4; class-variance-authority 0.7.1; clsx 2.1.1; tailwind-merge 3.6.0.
Visual assets: Lucide React 1.24.0 (matching the Verve reference) and self-hosted
Inter Variable 5.2.8. Fonts require no third-party request at runtime.

Exact tooling pins: Tailwind CSS and its Vite plugin 4.3.3; TypeScript 7.0.2;
Vite 8.2.2; React Vite plugin 6.1.1; Vitest 5.0.0; Biome 2.5.12;
Testing Library React 16.3.3; jest-dom 7.0.1; jsdom 30.0.1;
React types 19.2.18; React DOM types 19.2.7; Bun types 1.4.1.

The two source-owned UI components were generated with shadcn CLI 4.21.0
base-nova and use Base UI exclusively. No shadcn runtime package, Radix primitive
stack, or alternative package-manager lock is installed. See
`THIRD-PARTY-NOTICES.md` and `../docs/monitoring-references.md` for provenance.

`src/lib/api.ts` validates every response before rendering. Domain status,
readiness, aggregates, and execution eligibility come from Rust. The frontend
only places the backend's topologically ordered tasks into dependency stages.
Markdown keeps all textual contract content, shifts headings beneath the page
title, drops raw HTML and unsafe links, and renders images as text to avoid
unsolicited remote requests. No task editing or drag-and-drop status mutation.

## Verve redesign preview

The shell and all five screens use the `extra/verve-v1.1.0` visual basis:
63px icon rail, 200px inner navigation, 50px header, neutral light/dark tokens,
compact Inter typography, inset frames, and status accents. Sidebar collapse,
theme controls, refresh, navigation, and README disclosures are functional.
Backend contracts and task execution behavior are unchanged.

With `bun run dev`, visit `/design` to review the same screen components with
synthetic default, empty, loading, and error states. Design routes do not poll
the backend or execute tasks; production builds disable them unless explicitly
built with `VITE_DESIGN_ROUTES=1`. The design skill keeps approval separate from
implementation: `src/design/MANIFEST.md` records draft status and the review
matrix. No visual approval or screenshot baseline is inferred from passing tests.
