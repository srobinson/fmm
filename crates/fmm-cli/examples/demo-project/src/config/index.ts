import { config } from 'dotenv';

export interface AppConfig {
  port: number;
  dbUrl: string;
  jwtSecret: string;
  sessionTtl: number;
}

export function loadConfig(): AppConfig {
  config();
  return {
    port: parseInt(process.env.PORT || '3000'),
    dbUrl: process.env.DATABASE_URL || 'postgres://localhost/demo',
    jwtSecret: process.env.JWT_SECRET || 'dev-secret',
    sessionTtl: parseInt(process.env.SESSION_TTL || '3600'),
  };
}

export const DEFAULT_CONFIG: AppConfig = loadConfig();
