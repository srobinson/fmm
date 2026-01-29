// --- FMM ---
// fmm: v0.2
// file: api/models/session.ts
// exports: [Session, cleanExpiredSessions, createSession, deleteSessionsByUser, findSessionByToken]
// dependencies: [../../utils/id]
// loc: 54
// modified: 2026-01-29
// ---

import { generateId } from '../../utils/id';

export interface Session {
  id: string;
  userId: string;
  token: string;
  expiresAt: Date;
  createdAt: Date;
  ipAddress: string;
  userAgent: string;
}

const sessions = new Map<string, Session>();

export async function createSession(userId: string, token: string, meta: { ip: string; userAgent: string }): Promise<Session> {
  const session: Session = {
    id: generateId(),
    userId,
    token,
    expiresAt: new Date(Date.now() + 24 * 60 * 60 * 1000),
    createdAt: new Date(),
    ipAddress: meta.ip,
    userAgent: meta.userAgent,
  };
  sessions.set(session.id, session);
  return session;
}

export async function findSessionByToken(token: string): Promise<Session | undefined> {
  return Array.from(sessions.values()).find(s => s.token === token && s.expiresAt > new Date());
}

export async function deleteSessionsByUser(userId: string): Promise<number> {
  let count = 0;
  for (const [id, session] of sessions) {
    if (session.userId === userId) {
      sessions.delete(id);
      count++;
    }
  }
  return count;
}

export async function cleanExpiredSessions(): Promise<number> {
  const now = new Date();
  let count = 0;
  for (const [id, session] of sessions) {
    if (session.expiresAt < now) {
      sessions.delete(id);
      count++;
    }
  }
  return count;
}
