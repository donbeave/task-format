import { FolderKanban, Inbox } from "lucide-react";
import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  type Eligibility,
  runScope,
  type Scope,
  type Status,
  type Summary,
  type Task,
  taskHref,
} from "../lib/api";
import { useCatalog } from "../lib/catalog-context";
import { Button } from "./ui/button";
import { Progress } from "./ui/progress";
export const labels: Record<Status, string> = {
  draft: "Draft",
  pending: "Pending",
  in_progress: "In Progress",
  done: "Done",
};
export function Readme({ children }: { children: string }) {
  return (
    <div className="markdown">
      <Markdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          pre: ({ children }) => (
            // biome-ignore lint/a11y/noNoninteractiveTabindex: code blocks scroll horizontally on narrow screens.
            // biome-ignore lint/a11y/useSemanticElements: preserve preformatted code semantics while naming its scroll region.
            <pre role="region" tabIndex={0} aria-label="Task code block">
              {children}
            </pre>
          ),
          input: ({ checked }) => (
            <span
              role="img"
              aria-label={
                checked
                  ? "Completed checklist item"
                  : "Incomplete checklist item"
              }
            >
              {checked ? "☑" : "☐"}{" "}
            </span>
          ),
          h1: ({ children }) => <h2>{children}</h2>,
          h2: ({ children }) => <h3>{children}</h3>,
          h3: ({ children }) => <h4>{children}</h4>,
          a: ({ href, children }) =>
            href ? <a href={href}>{children}</a> : <span>{children}</span>,
          img: ({ alt }) => <span>[Image: {alt || "attachment"}]</span>,
        }}
      >
        {children}
      </Markdown>
    </div>
  );
}
export function ProgressMeter({
  value,
  label = "Checklist progress",
}: {
  value: number;
  label?: string;
}) {
  return (
    <div className="progress-meter">
      <div className="progress-label">
        <span>{label}</span>
        <span>{Math.round(value)}%</span>
      </div>
      <Progress value={value} aria-label={label} />
    </div>
  );
}
export function Badge({
  status,
  blocked = false,
}: {
  status: Status;
  blocked?: boolean;
}) {
  return (
    <span className={`badge ${status} ${blocked ? "blocked" : ""}`}>
      <span className="status-dot" aria-hidden="true" />
      {blocked ? "Blocked" : labels[status]}
    </span>
  );
}
export function Counts({ summary }: { summary: Summary }) {
  return (
    <div className="counts">
      {(["draft", "pending", "in_progress", "blocked", "done"] as const).map(
        (key) => (
          <span key={key} className={`count-item ${key}`}>
            <span className="status-dot" aria-hidden="true" />
            <strong>{summary[key]}</strong>{" "}
            {key === "blocked" ? "blocked" : labels[key].toLowerCase()}
          </span>
        ),
      )}
    </div>
  );
}
export function Stats({
  summary,
  projects,
}: {
  summary: Summary;
  projects?: number;
}) {
  return (
    <div className="stats">
      {projects !== undefined && (
        <div className="stat">
          <div className="stat-head">
            <span>Projects</span>
            <FolderKanban
              className="stat-symbol"
              size={16}
              aria-hidden="true"
            />
          </div>
          <strong className="stat-value">{projects}</strong>
          <span className="stat-caption">In your workspace</span>
        </div>
      )}
      {(["draft", "pending", "in_progress", "blocked", "done"] as const).map(
        (key) => (
          <div className={`stat ${key}`} key={key}>
            <div className="stat-head">
              <span>{key === "blocked" ? "Blocked" : labels[key]}</span>
              <span className="status-dot" aria-hidden="true" />
            </div>
            <strong className="stat-value">{summary[key]}</strong>
            <span className="stat-caption">
              {key === "draft"
                ? "Not yet scheduled"
                : key === "pending"
                  ? "Waiting to start"
                  : key === "in_progress"
                    ? "Currently executing"
                    : key === "blocked"
                      ? "Dependencies unresolved"
                      : "Verified complete"}
            </span>
          </div>
        ),
      )}
    </div>
  );
}
export function RunAction({
  scope,
  eligibility,
  label,
}: {
  scope: Scope;
  eligibility: Eligibility;
  label: string;
}) {
  const { state, refresh } = useCatalog();
  const [result, setResult] = useState<{
    kind: "idle" | "running" | "success" | "error";
    message: string;
  }>({ kind: "idle", message: "" });
  const request = useRef<AbortController | null>(null);
  const reasonId = useId();
  useEffect(() => () => request.current?.abort(), []);
  const reason =
    state.status === "ready" && state.refreshError
      ? "Execution disabled until fresh catalog data is available."
      : !eligibility.allowed
        ? eligibility.reason || "Execution is currently unavailable."
        : null;
  async function start() {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setResult({ kind: "running", message: "Starting execution…" });
    try {
      await runScope(scope, controller.signal);
      if (!controller.signal.aborted) {
        setResult({
          kind: "success",
          message: "Execution accepted. Progress refreshes automatically.",
        });
        refresh();
      }
    } catch (error) {
      if (!controller.signal.aborted) {
        setResult({
          kind: "error",
          message: error instanceof Error ? error.message : "Execution failed",
        });
        refresh();
      }
    }
  }
  return (
    <div className="action">
      <Button
        size="lg"
        disabled={Boolean(reason) || result.kind === "running"}
        aria-describedby={reason ? reasonId : undefined}
        onClick={() => void start()}
      >
        {result.kind === "running" ? "Starting…" : label}
      </Button>
      {reason && (
        <p id={reasonId} className="action-reason">
          {reason}
        </p>
      )}
      {result.kind !== "idle" && (
        <p
          role={result.kind === "error" ? "alert" : "status"}
          className="inline-message"
        >
          {result.message}
        </p>
      )}
    </div>
  );
}
export function TaskCard({
  task,
  children,
}: {
  task: Task;
  children?: ReactNode;
}) {
  const blocked = task.metadata.status === "pending" && !task.readiness.allowed;
  return (
    <article className={`task-card ${blocked ? "blocked" : ""}`}>
      <a className="task-card-body" href={taskHref(task)}>
        <div className="panel-head">
          <span className="task-id">{task.id}</span>
          <Badge status={task.metadata.status} blocked={blocked} />
        </div>
        <h3>{task.title}</h3>
        <ProgressMeter value={task.progress.percentage} />
        <p className="task-readiness muted">
          {task.metadata.status === "pending"
            ? task.readiness.allowed
              ? "Ready now"
              : task.readiness.reason
            : task.metadata.status === "in_progress"
              ? "Running"
              : task.metadata.status === "done"
                ? "Verified complete"
                : "Excluded from execution"}
        </p>
        {task.progress.current_leaf && (
          <p className="muted">Current: {task.progress.current_leaf}</p>
        )}
      </a>
      {children && <div className="task-card-footer">{children}</div>}
    </article>
  );
}
export function PageHeading({
  eyebrow,
  title,
  description,
  children,
}: {
  eyebrow: string;
  title: string;
  description?: string;
  children?: ReactNode;
}) {
  return (
    <header className="page-heading">
      <div className="heading-copy">
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        {description && <p className="lead">{description}</p>}
      </div>
      {children}
    </header>
  );
}
export function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="empty">
      <Inbox className="empty-icon" size={24} aria-hidden="true" />
      <div>{children}</div>
    </div>
  );
}
