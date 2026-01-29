// --- FMM ---
// fmm: v0.2
// file: api/models/user.ts
// exports: [User, countUsers, createUser, deleteUser, findUserByEmail, findUserById, listUsers, updateUser]
// dependencies: [../../auth/types, ../../utils/id]
// loc: 63
// modified: 2026-01-29
// ---

import { UserRole } from '../../auth/types';
import { generateId } from '../../utils/id';

export interface User {
  id: string;
  email: string;
  name: string;
  passwordHash: string;
  role: UserRole;
  createdAt: Date;
  updatedAt: Date;
}

interface CreateUserInput {
  email: string;
  name: string;
  passwordHash: string;
  role: UserRole;
}

const users = new Map<string, User>();

export async function createUser(input: CreateUserInput): Promise<User> {
  const user: User = {
    id: generateId(),
    email: input.email,
    name: input.name,
    passwordHash: input.passwordHash,
    role: input.role,
    createdAt: new Date(),
    updatedAt: new Date(),
  };
  users.set(user.id, user);
  return user;
}

export async function findUserByEmail(email: string): Promise<User | undefined> {
  return Array.from(users.values()).find(u => u.email === email);
}

export async function findUserById(id: string): Promise<User | undefined> {
  return users.get(id);
}

export async function listUsers(): Promise<User[]> {
  return Array.from(users.values());
}

export async function updateUser(id: string, updates: Partial<Pick<User, 'name' | 'email' | 'role'>>): Promise<User> {
  const user = users.get(id);
  if (!user) throw new Error(`User ${id} not found`);
  const updated = { ...user, ...updates, updatedAt: new Date() };
  users.set(id, updated);
  return updated;
}

export async function deleteUser(id: string): Promise<void> {
  users.delete(id);
}

export async function countUsers(): Promise<number> {
  return users.size;
}
