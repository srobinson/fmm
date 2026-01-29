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
