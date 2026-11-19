export type ClassValue =
  | ClassValue[]
  | Record<string, boolean | number | string | null | undefined>
  | string
  | number
  | boolean
  | null
  | undefined;

/**
 * High-performance, zero-dependency class name merger for modern React components.
 * Flattens strings, arrays, objects, and conditional expressions with zero vendor-chunk bundling bugs.
 */
export function cn(...inputs: ClassValue[]): string {
  const classes: string[] = [];

  for (const input of inputs) {
    if (!input) continue;

    if (typeof input === "string" || typeof input === "number") {
      classes.push(String(input));
    } else if (Array.isArray(input)) {
      const inner = cn(...input);
      if (inner) classes.push(inner);
    } else if (typeof input === "object") {
      for (const [key, value] of Object.entries(input)) {
        if (value) {
          classes.push(key);
        }
      }
    }
  }

  return classes.join(" ");
}

export function formatNumber(num: number): string {
  return new Intl.NumberFormat("en-US").format(num);
}

export function formatCurrency(amount: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  }).format(amount);
}

export function formatMicroseconds(us: number): string {
  if (us < 1000) {
    return `${us} µs`;
  }
  return `${(us / 1000).toFixed(2)} ms`;
}

/**
 * Safely extracts a numeric monotonic epoch from arbitrary vector clock representations.
 * Handles:
 *  - { clocks: { [nodeId]: epoch } } (Sunborn Rust VectorClock schema)
 *  - { [nodeId]: epoch } (Flat node map)
 *  - { epoch: number }
 *  - number
 * Falls back safely to fallback (default 1) if not present or malformed,
 * guaranteeing no raw object is ever rendered into React children.
 */
export function extractVectorClockEpoch(vc: unknown, fallback: number = 1): number {
  if (typeof vc === "number" && !Number.isNaN(vc)) {
    return vc;
  }
  if (!vc || typeof vc !== "object") {
    return fallback;
  }
  const obj = vc as Record<string, unknown>;
  if (typeof obj.epoch === "number" && !Number.isNaN(obj.epoch)) {
    return obj.epoch;
  }
  const targetMap =
    obj.clocks && typeof obj.clocks === "object"
      ? (obj.clocks as Record<string, unknown>)
      : obj;

  const numericValues = Object.values(targetMap).filter(
    (v): v is number => typeof v === "number" && !Number.isNaN(v)
  );
  if (numericValues.length > 0) {
    return Math.max(...numericValues);
  }
  return fallback;
}

