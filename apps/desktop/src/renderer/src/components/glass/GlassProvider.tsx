import { createContext, onMount, useContext, type JSX } from "solid-js";
import { createLiquidGlass, type LiquidGlassOptions } from "solid-glass/engines/svg-refraction";

interface GlassContextValue {
  filterRefForSize: (width: number, height: number) => string;
}

const GLASS_SPEC = {
  radius: 60,
  bezelWidth: 15,
  glassThickness: 200,
  blur: 0,
  refractiveIndex: 1.5,
  surface: "convexSquircle",
  specularOpacity: 0.6
} as const;

const GLASS_CSS_VARS = {
  "--glass-radius": `${GLASS_SPEC.radius}px`,
  "--glass-bezel-width": `${GLASS_SPEC.bezelWidth}px`,
  "--glass-thickness": `${GLASS_SPEC.glassThickness}px`,
  "--glass-blur": `${GLASS_SPEC.blur}px`,
  "--glass-refractive-index": String(GLASS_SPEC.refractiveIndex),
  "--glass-specular-opacity": String(GLASS_SPEC.specularOpacity)
};

const GlassContext = createContext<GlassContextValue>();
const filterCache = new Map<string, string>();

function applyGlassCssVars(): void {
  for (const [name, value] of Object.entries(GLASS_CSS_VARS)) {
    document.documentElement.style.setProperty(name, value);
  }
}

function normalizeSize(value: number): number {
  return Math.max(1, Math.round(value));
}

export function ensureGlassFilter(width: number, height: number): string {
  const normalizedWidth = normalizeSize(width);
  const normalizedHeight = normalizeSize(height);
  const cacheKey = `${normalizedWidth}x${normalizedHeight}`;
  const cached = filterCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const glass = createLiquidGlass({
    width: normalizedWidth,
    height: normalizedHeight,
    ...GLASS_SPEC
  } satisfies LiquidGlassOptions);

  document.body.insertAdjacentHTML("afterbegin", glass.svgFilter);
  filterCache.set(cacheKey, glass.filterRef);
  return glass.filterRef;
}

export function GlassProvider(props: { children: JSX.Element }): JSX.Element {
  onMount(() => {
    applyGlassCssVars();
  });

  return (
    <GlassContext.Provider value={{ filterRefForSize: ensureGlassFilter }}>
      <div class="glass-provider-root" style={GLASS_CSS_VARS as JSX.CSSProperties}>
        {props.children}
      </div>
    </GlassContext.Provider>
  );
}

export function useGlass(): GlassContextValue {
  const value = useContext(GlassContext);
  if (!value) {
    throw new Error("Glass components must be rendered inside GlassProvider.");
  }
  return value;
}
