import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { type Catalog, fetchCatalog } from "./api";

type State =
  | { readonly status: "loading" }
  | { readonly status: "error"; readonly message: string }
  | {
      readonly status: "ready";
      readonly catalog: Catalog;
      readonly refreshError: string | null;
    };
const Context = createContext<{ state: State; refresh: () => void } | null>(
  null,
);
export function CatalogProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<State>({ status: "loading" });
  const [revision, setRevision] = useState(0);
  const refresh = useCallback(() => setRevision((x) => x + 1), []);
  const current = useRef<Catalog | null>(null);
  // A manual refresh intentionally replaces the current request and polling timer.
  // biome-ignore lint/correctness/useExhaustiveDependencies: revision is the explicit restart signal.
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      try {
        const catalog = await fetchCatalog(controller.signal);
        if (!controller.signal.aborted) {
          current.current = catalog;
          setState({ status: "ready", catalog, refreshError: null });
        }
      } catch (error) {
        if (!controller.signal.aborted) {
          const message =
            error instanceof Error
              ? error.message
              : "Unable to reach the backend";
          setState(
            current.current
              ? {
                  status: "ready",
                  catalog: current.current,
                  refreshError: message,
                }
              : { status: "error", message },
          );
        }
      } finally {
        if (!controller.signal.aborted)
          timer = setTimeout(() => {
            void poll();
          }, 3000);
      }
    }
    void poll();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [revision]);
  return <Context value={{ state, refresh }}>{children}</Context>;
}
export function useCatalog() {
  const value = useContext(Context);
  if (!value) throw new Error("Catalog provider missing");
  return value;
}
