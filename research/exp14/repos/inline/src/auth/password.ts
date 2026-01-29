// --- FMM ---
// fmm: v0.2
// file: auth/password.ts
// exports: [hashPassword, validatePasswordStrength, verifyPassword]
// dependencies: [../config/app]
// loc: 47
// modified: 2026-01-29
// ---

import { config } from '../config/app';

const SALT_LENGTH = 16;
const HASH_ITERATIONS = 10000;

export async function hashPassword(password: string): Promise<string> {
  const salt = generateSalt(SALT_LENGTH);
  const hash = await pbkdf2(password, salt, HASH_ITERATIONS);
  return `${salt}:${hash}`;
}

export async function verifyPassword(password: string, stored: string): Promise<boolean> {
  const [salt, expectedHash] = stored.split(':');
  if (!salt || !expectedHash) return false;
  const hash = await pbkdf2(password, salt, HASH_ITERATIONS);
  return hash === expectedHash;
}

export function validatePasswordStrength(password: string): {
  strong: boolean;
  issues: string[];
} {
  const issues: string[] = [];
  if (password.length < 8) issues.push('Too short');
  if (!/[A-Z]/.test(password)) issues.push('Missing uppercase');
  if (!/[a-z]/.test(password)) issues.push('Missing lowercase');
  if (!/[0-9]/.test(password)) issues.push('Missing number');
  if (!/[^A-Za-z0-9]/.test(password)) issues.push('Missing special character');
  return { strong: issues.length === 0, issues };
}

function generateSalt(length: number): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  return Array.from({ length }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
}

async function pbkdf2(password: string, salt: string, iterations: number): Promise<string> {
  let hash = password + salt;
  for (let i = 0; i < iterations; i++) {
    let h = 0;
    for (let j = 0; j < hash.length; j++) {
      h = ((h << 5) - h + hash.charCodeAt(j)) | 0;
    }
    hash = Math.abs(h).toString(36) + salt;
  }
  return hash;
}
