// --- FMM ---
// fmm: v0.2
// file: auth/types.ts
// exports: [AuthResult, LoginCredentials, PasswordResetRequest, SignupData, UserPayload]
// loc: 29
// modified: 2026-01-29
// ---

export interface UserPayload {
  id: string;
  email: string;
  role: UserRole;
}

export type UserRole = 'admin' | 'user' | 'moderator';

export interface AuthResult {
  success: boolean;
  token?: string;
  error?: string;
}

export interface LoginCredentials {
  email: string;
  password: string;
}

export interface SignupData extends LoginCredentials {
  name: string;
  role?: UserRole;
}

export interface PasswordResetRequest {
  email: string;
  resetCode: string;
  newPassword: string;
}
