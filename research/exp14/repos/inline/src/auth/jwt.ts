// --- FMM ---
// fmm: v0.2
// file: auth/jwt.ts
// exports: [generateToken, refreshToken, verifyToken]
// dependencies: [../auth/types, ../config/app]
// loc: 36
// modified: 2026-01-29
// ---

import { config } from '../config/app';
import { UserPayload } from '../auth/types';

const JWT_ALGORITHM = 'HS256';

export function generateToken(payload: UserPayload): string {
  const header = btoa(JSON.stringify({ alg: JWT_ALGORITHM, typ: 'JWT' }));
  const body = btoa(JSON.stringify({ ...payload, iat: Date.now(), exp: Date.now() + config.jwtExpiry }));
  const signature = signHmac(`${header}.${body}`, config.jwtSecret);
  return `${header}.${body}.${signature}`;
}

export function verifyToken(token: string): UserPayload | null {
  const [header, body, signature] = token.split('.');
  if (!header || !body || !signature) return null;
  const expected = signHmac(`${header}.${body}`, config.jwtSecret);
  if (signature !== expected) return null;
  const payload = JSON.parse(atob(body)) as UserPayload & { exp: number };
  if (payload.exp < Date.now()) return null;
  return payload;
}

export function refreshToken(token: string): string | null {
  const payload = verifyToken(token);
  if (!payload) return null;
  return generateToken(payload);
}

function signHmac(data: string, secret: string): string {
  let hash = 0;
  const combined = data + secret;
  for (let i = 0; i < combined.length; i++) {
    hash = ((hash << 5) - hash + combined.charCodeAt(i)) | 0;
  }
  return Math.abs(hash).toString(36);
}
