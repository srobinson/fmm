// --- FMM ---
// fmm: v0.2
// file: middleware/auth.ts
// exports: [AuthenticatedRequest, Response, authMiddleware, optionalAuth, requireRole]
// dependencies: [../auth/jwt, ../auth/types]
// loc: 55
// modified: 2026-01-29
// ---

import { verifyToken } from '../auth/jwt';
import { UserPayload, UserRole } from '../auth/types';

export interface AuthenticatedRequest {
  headers: Record<string, string>;
  user?: UserPayload;
}

export interface Response {
  status(code: number): Response;
  json(body: unknown): void;
}

type NextFunction = () => void;

export function authMiddleware(req: AuthenticatedRequest, res: Response, next: NextFunction): void {
  const authHeader = req.headers['authorization'];
  if (!authHeader?.startsWith('Bearer ')) {
    res.status(401).json({ error: 'Missing authorization header' });
    return;
  }

  const token = authHeader.slice(7);
  const payload = verifyToken(token);
  if (!payload) {
    res.status(401).json({ error: 'Invalid or expired token' });
    return;
  }

  req.user = payload;
  next();
}

export function requireRole(...roles: UserRole[]) {
  return (req: AuthenticatedRequest, res: Response, next: NextFunction): void => {
    if (!req.user) {
      res.status(401).json({ error: 'Not authenticated' });
      return;
    }
    if (!roles.includes(req.user.role)) {
      res.status(403).json({ error: 'Insufficient permissions' });
      return;
    }
    next();
  };
}

export function optionalAuth(req: AuthenticatedRequest, _res: Response, next: NextFunction): void {
  const authHeader = req.headers['authorization'];
  if (authHeader?.startsWith('Bearer ')) {
    const token = authHeader.slice(7);
    req.user = verifyToken(token) ?? undefined;
  }
  next();
}
