import { createSignal, onCleanup, onMount, splitProps, type Accessor, type JSX } from "solid-js";
import { useGlass } from "./GlassProvider";

type DivProps = JSX.HTMLAttributes<HTMLDivElement>;
type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement>;

function mergeClass(base: string, extra?: string): string {
  return extra ? `${base} ${extra}` : base;
}

function callRef<T>(ref: unknown, element: T): void {
  if (typeof ref === "function") {
    (ref as (element: T) => void)(element);
  }
}

function glassStyle(filterRef: string, style: JSX.CSSProperties | string | undefined): JSX.CSSProperties {
  return {
    "--panel-filter": filterRef,
    ...(typeof style === "object" ? style : {})
  } as JSX.CSSProperties;
}

function useMeasuredGlassFilter<T extends HTMLElement>(): {
  bind: (node: T, forwardedRef: unknown) => void;
  filterRef: Accessor<string>;
} {
  let element: T | undefined;
  const [filterRef, setFilterRef] = createSignal("none");
  const glass = useGlass();

  onMount(() => {
    if (!element) {
      return;
    }

    const updateFilter = (): void => {
      const rect = element?.getBoundingClientRect();
      if (rect) {
        setFilterRef(glass.filterRefForSize(rect.width, rect.height));
      }
    };

    updateFilter();
    const observer = new ResizeObserver(updateFilter);
    observer.observe(element);
    onCleanup(() => observer.disconnect());
  });

  return {
    bind: (node, forwardedRef) => {
      element = node;
      callRef(forwardedRef, node);
    },
    filterRef
  };
}

export function GlassPanel(props: DivProps): JSX.Element {
  const surface = useMeasuredGlassFilter<HTMLDivElement>();
  const [local, rest] = splitProps(props, ["class", "style", "ref"]);

  return (
    <div
      {...rest}
      ref={(node) => surface.bind(node, local.ref)}
      class={mergeClass("glass-panel", local.class)}
      style={glassStyle(surface.filterRef(), local.style)}
    />
  );
}

export function GlassButton(props: ButtonProps): JSX.Element {
  const surface = useMeasuredGlassFilter<HTMLButtonElement>();
  const [local, rest] = splitProps(props, ["class", "style", "ref"]);

  return (
    <button
      {...rest}
      ref={(node) => surface.bind(node, local.ref)}
      class={mergeClass("glass-button", local.class)}
      style={glassStyle(surface.filterRef(), local.style)}
    />
  );
}

export function GlassToolbar(props: DivProps): JSX.Element {
  const surface = useMeasuredGlassFilter<HTMLDivElement>();
  const [local, rest] = splitProps(props, ["class", "style", "ref"]);

  return (
    <div
      {...rest}
      ref={(node) => surface.bind(node, local.ref)}
      class={mergeClass("glass-toolbar", local.class)}
      style={glassStyle(surface.filterRef(), local.style)}
    />
  );
}

export function GlassDialog(props: DivProps): JSX.Element {
  const surface = useMeasuredGlassFilter<HTMLDivElement>();
  const [local, rest] = splitProps(props, ["class", "style", "ref"]);

  return (
    <div
      {...rest}
      ref={(node) => surface.bind(node, local.ref)}
      class={mergeClass("glass-dialog", local.class)}
      style={glassStyle(surface.filterRef(), local.style)}
    />
  );
}
