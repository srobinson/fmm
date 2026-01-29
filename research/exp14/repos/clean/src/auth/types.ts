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
