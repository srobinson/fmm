import { Router, Request, Response } from 'express';
import { requireAuth, AuthenticatedRequest } from '../auth/middleware';
import { createSession } from '../auth/session';
import { findUserByEmail, createUser } from '../db/users';
import { DatabaseClient } from '../db/client';
import { DEFAULT_CONFIG } from '../config';
import bcrypt from 'bcrypt';

export function createRouter(db: DatabaseClient): Router {
  const router = Router();

  router.post('/auth/login', async (req: Request, res: Response) => {
    const { email, password } = req.body;
    const user = await findUserByEmail(db, email);
    if (!user || !await bcrypt.compare(password, user.passwordHash)) {
      res.status(401).json({ error: 'Invalid credentials' });
      return;
    }
    const token = createSession(user, DEFAULT_CONFIG);
    res.json({ token });
  });

  router.post('/auth/register', async (req: Request, res: Response) => {
    const { email, password } = req.body;
    const hash = await bcrypt.hash(password, 10);
    const user = await createUser(db, email, hash);
    const token = createSession(user, DEFAULT_CONFIG);
    res.status(201).json({ token });
  });

  router.get('/me', requireAuth, (req: AuthenticatedRequest, res: Response) => {
    res.json({ userId: req.userId, email: req.email });
  });

  return router;
}
