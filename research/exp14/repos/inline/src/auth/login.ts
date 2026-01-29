// --- FMM ---
// fmm: v0.2
// file: auth/login.ts
// exports: [authenticateUser, validateSession]
// dependencies: [../api/models/user, ../services/audit, ./jwt, ./password, ./types]
// loc: 30
// modified: 2026-01-29
// ---

import { LoginCredentials, AuthResult, UserPayload } from './types';
import { generateToken } from './jwt';
import { findUserByEmail } from '../api/models/user';
import { verifyPassword } from './password';
import { logAuthEvent } from '../services/audit';

export async function authenticateUser(credentials: LoginCredentials): Promise<AuthResult> {
  const user = await findUserByEmail(credentials.email);
  if (!user) {
    await logAuthEvent('login_failed', { email: credentials.email, reason: 'user_not_found' });
    return { success: false, error: 'Invalid credentials' };
  }

  const valid = await verifyPassword(credentials.password, user.passwordHash);
  if (!valid) {
    await logAuthEvent('login_failed', { email: credentials.email, reason: 'wrong_password' });
    return { success: false, error: 'Invalid credentials' };
  }

  const payload: UserPayload = { id: user.id, email: user.email, role: user.role };
  const token = generateToken(payload);
  await logAuthEvent('login_success', { userId: user.id });

  return { success: true, token };
}

export async function validateSession(token: string): Promise<UserPayload | null> {
  const { verifyToken } = await import('./jwt');
  return verifyToken(token);
}
