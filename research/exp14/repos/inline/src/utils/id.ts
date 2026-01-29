// --- FMM ---
// fmm: v0.2
// file: utils/id.ts
// exports: [extractTimestamp, generateId, isValidId]
// loc: 18
// modified: 2026-01-29
// ---

let counter = 0;

export function generateId(): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substring(2, 8);
  const seq = (counter++).toString(36);
  return `${timestamp}-${random}-${seq}`;
}

export function isValidId(id: string): boolean {
  return /^[a-z0-9]+-[a-z0-9]+-[a-z0-9]+$/.test(id);
}

export function extractTimestamp(id: string): number | null {
  const parts = id.split('-');
  if (parts.length < 1) return null;
  return parseInt(parts[0], 36);
}
