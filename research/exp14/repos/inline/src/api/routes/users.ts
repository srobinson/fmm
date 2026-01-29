// --- FMM ---
// fmm: v0.2
// file: api/routes/users.ts
// exports: [getUserRoutes]
// dependencies: [../../middleware/auth, ../models/user]
// loc: 74
// modified: 2026-01-29
// ---

import { findUserById, listUsers, updateUser, deleteUser } from '../models/user';
import { authMiddleware, requireRole, AuthenticatedRequest, Response } from '../../middleware/auth';

interface Route {
  method: string;
  path: string;
  middleware: Function[];
  handler: (req: AuthenticatedRequest, res: Response) => Promise<void>;
}

export function getUserRoutes(): Route[] {
  return [
    {
      method: 'GET',
      path: '/users',
      middleware: [authMiddleware, requireRole('admin')],
      handler: handleListUsers,
    },
    {
      method: 'GET',
      path: '/users/:id',
      middleware: [authMiddleware],
      handler: handleGetUser,
    },
    {
      method: 'PUT',
      path: '/users/:id',
      middleware: [authMiddleware],
      handler: handleUpdateUser,
    },
    {
      method: 'DELETE',
      path: '/users/:id',
      middleware: [authMiddleware, requireRole('admin')],
      handler: handleDeleteUser,
    },
  ];
}

async function handleListUsers(_req: AuthenticatedRequest, res: Response): Promise<void> {
  const users = await listUsers();
  res.status(200).json({ users: users.map(sanitizeUser) });
}

async function handleGetUser(req: AuthenticatedRequest, res: Response): Promise<void> {
  const id = (req as any).params?.id;
  const user = await findUserById(id);
  if (user) {
    res.status(200).json({ user: sanitizeUser(user) });
  } else {
    res.status(404).json({ error: 'User not found' });
  }
}

async function handleUpdateUser(req: AuthenticatedRequest, res: Response): Promise<void> {
  const id = (req as any).params?.id;
  if (req.user?.id !== id && req.user?.role !== 'admin') {
    res.status(403).json({ error: 'Cannot update other users' });
    return;
  }
  const updated = await updateUser(id, (req as any).body);
  res.status(200).json({ user: sanitizeUser(updated) });
}

async function handleDeleteUser(req: AuthenticatedRequest, res: Response): Promise<void> {
  const id = (req as any).params?.id;
  await deleteUser(id);
  res.status(204).json({});
}

function sanitizeUser(user: any): Record<string, unknown> {
  const { passwordHash, ...safe } = user;
  return safe;
}
