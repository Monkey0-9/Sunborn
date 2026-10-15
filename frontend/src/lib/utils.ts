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
