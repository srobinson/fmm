import { Request, Response, NextFunction } from 'express';
import { validateSession } from './session';
import { DEFAULT_CONFIG } from '../config';

export interface AuthenticatedRequest extends Request {
  userId?: string;
  email?: string;
}

export function requireAuth(req: AuthenticatedRequest, res: Response, next: NextFunction): void {
  const token = req.headers.authorization?.replace('Bearer ', '');
  if (!token) {
    res.status(401).json({ error: 'Authentication required' });
    return;
  }

  const session = validateSession(token, DEFAULT_CONFIG);
  if (!session) {
    res.status(401).json({ error: 'Invalid or expired token' });
    return;
  }

  req.userId = session.userId;
  req.email = session.email;
  next();
}
