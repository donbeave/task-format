# Monitoring reference provenance

The read-only inputs are `extra/verve-v1.1.0` and
`extra/tempo-v1.1.0`. The monitoring application independently implements the
concepts below against filesystem catalog data. Neither reference application,
its mock data, nor its assets should be copied into the application.

## Verve: visual reference

Paths in this section are relative to `extra/verve-v1.1.0/`.

| Inspected source | Concept selected | Monitoring destination |
| --- | --- | --- |
| `components/shared/app-shell/app-shell.tsx`, `app-shell-content.tsx`, `app-sidebar.tsx` | Narrow primary navigation, contextual navigation, compact header, inset bordered content surface | Shared application shell and project navigation |
| `components/shared/app-shell/routes.ts`, `nav-main.tsx`, `inner-sidebar-nav.tsx` | URL determines selection; current navigation link has `aria-current`; compact section labels | Dashboard, project, group, and My Tasks navigation |
| `components/shared/app-shell/inner-sidebar.tsx`, `components/ui/sidebar.tsx`, `hooks/use-mobile.ts` | Desktop navigation becomes a mobile drawer; content owns scrolling | Responsive shell |
| `app/globals.css`, `app/layout.tsx` | Neutral surfaces, compact sans typography, restrained status colors, rounded borders | Application design tokens and typography |
| `components/reui/frame.tsx` | Muted outer frame with bordered inner panels and concentric radii | Summary cards, group cards, Kanban columns, task detail sections |
| `features/home/components/metric-grid.tsx` | Responsive metric grid, clear count hierarchy, tabular numbers | Dashboard status aggregates |
| `features/pipeline/components/kanban-board.tsx` | Horizontally scrollable columns with status dot, title, count, compact cards, labeled progress | Group Draft/Pending/In Progress/Done board |
| `features/tasks/components/tasks.tsx`, `data-grid-view.tsx`, `columns.tsx` | Compact status labels and informative empty states | Task summaries and empty task views |
| `components/ui/button.tsx`, `progress.tsx`, `sidebar.tsx` | Base UI primitives beneath local styled components; visible focus and accessible labels | Source-owned monitoring UI primitives |
| `components/top-progress-bar.tsx`, `components/ui/skeleton.tsx`, `spinner.tsx` | Navigation feedback and reduced-motion consideration | Loading and refresh feedback |

The visual reference uses a 63px primary rail, a default 200px contextual pane,
a 50px header, 16px content padding, and centered content capped at 1280px.
Its Kanban columns are approximately 296px wide, with 12px card padding,
compact metadata, and thin progress tracks. These are visual reference
measurements, not requirements to reproduce every pixel or optional interaction.

Verve's task view is a flat table with expandable subissues, not a dependency
graph. Its blank hydration placeholder is not an adequate loading state for the
monitoring application. Creation, status dragging, assignees, CRM fields, and
mock action toasts are excluded; task lifecycle remains backend-controlled.

## Tempo: structure and dependency presentation reference

Paths in this section are relative to `extra/tempo-v1.1.0/`.

| Inspected source | Concept selected | Monitoring destination |
| --- | --- | --- |
| `vite.config.ts` | Tailwind, TanStack Start, and React plugin integration | Frontend build configuration, using current compatible versions |
| `src/router.tsx` | Router factory, generated file route tree, shared error handling | Monitoring router |
| `src/routes/__root.tsx` | Root document shell with `HeadContent` and `Scripts` | Root HTML and document metadata |
| `src/routes/_app.tsx` | Persistent application shell with outlet and skip link | Shared navigation and accessible main-content entry |
| `src/routes/_app/tasks/board.tsx` | Route loading feedback, `aria-busy`, error retry via invalidation | Catalog route loading/error states |
| `src/features/gantt/components/demo.tsx` | Framed resource hierarchy aligned with visual lanes | My Tasks dependency stages and linked task rows |

Tempo's Gantt model has resource children and dated events. It does not implement
a dependency DAG. The monitoring application must derive topological stages and
parallel readiness from validated dependency edges, and show prerequisites and
downstream tasks explicitly. Calendar dates, people, and resource nesting cannot
serve as substitutes for task dependency semantics.

## Implemented destinations

The inspected implementation uses these concrete paths:

- `web/src/components/shell.tsx`: persistent rail and project navigation,
  inset content, skip link, mobile disclosure navigation, catalog loading,
  refresh failure, and retry states. Mobile navigation uses native
  `details`/`summary`, adapting the drawer concept without copying it.
- `web/src/styles.css`: neutral surface tokens, compact typography, 64px rail,
  208px contextual sidebar, 50px header, nested frames, responsive grids,
  horizontally overflowing Kanban, and reduced-motion rules.
- `web/src/components/views.tsx`: dashboard metric/project cards, project/group
  summaries, four-column group board, task detail, and My Tasks topological
  lanes with explicit prerequisite and dependent links.
- `web/src/components/monitor.tsx`: shared status badges, counts, labeled
  progress, task cards, Markdown, backend-controlled run actions with disabled
  reasons, and empty states.
- `web/src/components/ui/button.tsx` and `progress.tsx`: source-owned Base UI
  wrappers generated from the official shadcn `base-nova` registry using
  `bunx --bun shadcn@4.21.0 add button progress card --yes`. Similarity to
  the reference wrappers comes from this shared upstream registry, not copying
  the proprietary template. The unused generated card is not needed by the
  application.
- `web/src/router.tsx`, `web/src/routes/__root.tsx`, and `web/src/routes/`:
  TanStack router factory, HTML document shell, and required filesystem-derived
  dashboard/project/group/task/My Tasks routes.
- `web/vite.config.ts`: current TanStack Start, React, and Tailwind integration.

The application uses its own task data and text. It omits source-template
avatars, commercial branding, mock charts, CRM actions, auth, and Gantt dates.
Loading feedback is semantic text rather than a copied route progress bar.

## License and reuse boundary

Both reference `LICENSE.md` files state copyright Keenthemes, all rights
reserved. They permit application use under a valid ReUI license and prohibit
redistributing the template or publishing it in whole or substantial part.
Their demo images and placeholder content are not licensed for reuse outside
the templates. Registry dependencies retain their separate licenses.

The implementation policy is independent code informed by the concepts above;
no reference source or asset is selected for copying. Reference notices remain
in their original read-only directories. Any later intentional source or asset
reuse requires recording its exact path, applicable notice, adaptation, and
verification here before inclusion. Reference dependency lockfiles are not a
source of dependency versions.
