// --- FMM ---
// fmm: v0.2
// file: api/routes/auth.ts
// exports: [getAuthRoutes]
// dependencies: [../../auth/jwt, ../../auth/login, ../../auth/signup, ../../middleware/auth, ../../middleware/rateLimit]
// loc: 84
// modified: 2026-01-29
// ---

import { authenticateUser, validateSession } from '../../auth/login';
import { registerUser, validateSignupData } from '../../auth/signup';
import { refreshToken } from '../../auth/jwt';
import { authMiddleware, AuthenticatedRequest, Response } from '../../middleware/auth';
import { authRateLimiter } from '../../middleware/rateLimit';

interface Route {
  method: string;
  path: string;
  middleware: Function[];
  handler: (req: AuthenticatedRequest, res: Response) => Promise<void>;
}

export function getAuthRoutes(): Route[] {
  return [
    {
      method: 'POST',
      path: '/auth/login',
      middleware: [authRateLimiter()],
      handler: handleLogin,
    },
    {
      method: 'POST',
      path: '/auth/signup',
      middleware: [authRateLimiter()],
      handler: handleSignup,
    },
    {
      method: 'POST',
      path: '/auth/refresh',
      middleware: [authMiddleware],
      handler: handleRefresh,
    },
    {
      method: 'GET',
      path: '/auth/me',
      middleware: [authMiddleware],
      handler: handleMe,
    },
  ];
}

async function handleLogin(req: AuthenticatedRequest, res: Response): Promise<void> {
  const { email, password } = req.headers as any;
  const result = await authenticateUser({ email, password });
  if (result.success) {
    res.status(200).json({ token: result.token });
  } else {
    res.status(401).json({ error: result.error });
  }
}

async function handleSignup(req: AuthenticatedRequest, res: Response): Promise<void> {
  const data = req.headers as any;
  const errors = validateSignupData(data);
  if (errors.length > 0) {
    res.status(400).json({ errors });
    return;
  }
  const result = await registerUser(data);
  if (result.success) {
    res.status(201).json({ token: result.token });
  } else {
    res.status(409).json({ error: result.error });
  }
}

async function handleRefresh(req: AuthenticatedRequest, res: Response): Promise<void> {
  const token = req.headers['authorization']?.slice(7) || '';
  const newToken = refreshToken(token);
  if (newToken) {
    res.status(200).json({ token: newToken });
  } else {
    res.status(401).json({ error: 'Could not refresh token' });
  }
}

async function handleMe(req: AuthenticatedRequest, res: Response): Promise<void> {
  if (req.user) {
    res.status(200).json({ user: req.user });
  } else {
    res.status(401).json({ error: 'Not authenticated' });
  }
}
