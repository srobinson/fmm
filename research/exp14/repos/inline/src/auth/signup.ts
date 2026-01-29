// --- FMM ---
// fmm: v0.2
// file: auth/signup.ts
// exports: [registerUser, validateSignupData]
// dependencies: [../api/models/user, ../services/audit, ../services/email, ./jwt, ./password, ./types]
// loc: 37
// modified: 2026-01-29
// ---

import { SignupData, AuthResult, UserPayload } from './types';
import { generateToken } from './jwt';
import { createUser, findUserByEmail } from '../api/models/user';
import { hashPassword } from './password';
import { logAuthEvent } from '../services/audit';
import { sendWelcomeEmail } from '../services/email';

export async function registerUser(data: SignupData): Promise<AuthResult> {
  const existing = await findUserByEmail(data.email);
  if (existing) {
    return { success: false, error: 'Email already registered' };
  }

  const passwordHash = await hashPassword(data.password);
  const user = await createUser({
    email: data.email,
    name: data.name,
    passwordHash,
    role: data.role || 'user',
  });

  await logAuthEvent('signup', { userId: user.id });
  await sendWelcomeEmail(user.email, user.name);

  const payload: UserPayload = { id: user.id, email: user.email, role: user.role };
  const token = generateToken(payload);

  return { success: true, token };
}

export function validateSignupData(data: Partial<SignupData>): string[] {
  const errors: string[] = [];
  if (!data.email?.includes('@')) errors.push('Invalid email');
  if (!data.password || data.password.length < 8) errors.push('Password must be at least 8 characters');
  if (!data.name?.trim()) errors.push('Name is required');
  return errors;
}
