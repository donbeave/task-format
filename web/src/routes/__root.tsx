import { createRootRoute, HeadContent, Scripts } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { Shell } from "../components/shell";
import { CatalogProvider } from "../lib/catalog-context";
import stylesheet from "../styles.css?url";
export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: "Task monitor" },
    ],
    links: [
      { rel: "stylesheet", href: stylesheet },
      {
        rel: "icon",
        href: 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"%3E%3Crect width="32" height="32" rx="8" fill="%23252a32"/%3E%3Ctext x="10" y="24" fill="white" font-size="26" font-family="sans-serif"%3Et%3C/text%3E%3C/svg%3E',
      },
    ],
  }),
  component: () => (
    <CatalogProvider>
      <Shell />
    </CatalogProvider>
  ),
  shellComponent: Document,
  errorComponent: ({ error, reset }) => (
    <main className="content">
      <h1>Unable to display this page</h1>
      <p role="alert">
        {error instanceof Error
          ? error.message
          : "Unexpected rendering failure"}
      </p>
      <button type="button" onClick={reset}>
        Retry
      </button>
    </main>
  ),
});
function Document({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}
