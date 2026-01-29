// --- FMM ---
// fmm: v0.2
// file: api/controllers/authController.ts
// exports: [AuthController, createAuthController]
// dependencies: [../../auth/login, ../../auth/signup, ../../auth/types, ../../services/audit]
// loc: 31
// modified: 2026-01-29
// ---

import { authenticateUser } from '../../auth/login';
import { registerUser } from '../../auth/signup';
import { logAuthEvent } from '../../services/audit';
import { LoginCredentials, SignupData } from '../../auth/types';

export class AuthController {
  async login(credentials: LoginCredentials) {
    const result = await authenticateUser(credentials);
    if (result.success) {
      await logAuthEvent('controller_login', { email: credentials.email });
    }
    return result;
  }

  async signup(data: SignupData) {
    const result = await registerUser(data);
    if (result.success) {
      await logAuthEvent('controller_signup', { email: data.email });
    }
    return result;
  }

  async logout(userId: string) {
    await logAuthEvent('logout', { userId });
    return { success: true };
  }
}

export function createAuthController(): AuthController {
  return new AuthController();
}
