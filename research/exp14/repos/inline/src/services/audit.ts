// --- FMM ---
// fmm: v0.2
// file: services/audit.ts
// exports: [AuditEntry, clearAuditLog, getAuditLog, logAuthEvent]
// loc: 38
// modified: 2026-01-29
// ---

export interface AuditEntry {
  timestamp: Date;
  event: string;
  data: Record<string, unknown>;
}

const auditLog: AuditEntry[] = [];

export async function logAuthEvent(event: string, data: Record<string, unknown>): Promise<void> {
  const entry: AuditEntry = {
    timestamp: new Date(),
    event,
    data,
  };
  auditLog.push(entry);
}

export async function getAuditLog(filters?: {
  event?: string;
  since?: Date;
  userId?: string;
}): Promise<AuditEntry[]> {
  let entries = [...auditLog];
  if (filters?.event) {
    entries = entries.filter(e => e.event === filters.event);
  }
  if (filters?.since) {
    entries = entries.filter(e => e.timestamp >= filters.since!);
  }
  if (filters?.userId) {
    entries = entries.filter(e => e.data.userId === filters.userId);
  }
  return entries;
}

export function clearAuditLog(): void {
  auditLog.length = 0;
}
