import jwt from 'jsonwebtoken';
import { AppConfig } from '../config';
import { User } from '../db/users';

export interface SessionPayload {
  userId: string;
  email: string;
  iat: number;
  exp: number;
}

export function createSession(user: User, config: AppConfig): string {
  return jwt.sign(
    { userId: user.id, email: user.email },
    config.jwtSecret,
    { expiresIn: config.sessionTtl }
  );
}

export function validateSession(token: string, config: AppConfig): SessionPayload | null {
  try {
    return jwt.verify(token, config.jwtSecret) as SessionPayload;
  } catch {
    return null;
  }
}

export function destroySession(_token: string): void {
  // In a real app, add token to a blocklist
}
