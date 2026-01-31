import { DatabaseClient } from './client';

export interface User {
  id: string;
  email: string;
  passwordHash: string;
  createdAt: Date;
}

export async function findUserByEmail(db: DatabaseClient, email: string): Promise<User | null> {
  const result = await db.query<User>('SELECT * FROM users WHERE email = $1', [email]);
  return result.rows[0] || null;
}

export async function createUser(db: DatabaseClient, email: string, passwordHash: string): Promise<User> {
  const result = await db.query<User>(
    'INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING *',
    [email, passwordHash]
  );
  return result.rows[0];
}

export async function deleteUser(db: DatabaseClient, id: string): Promise<boolean> {
  const result = await db.query('DELETE FROM users WHERE id = $1', [id]);
  return (result.rowCount ?? 0) > 0;
}
