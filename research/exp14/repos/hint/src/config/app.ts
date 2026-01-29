export interface AppConfig {
  port: number;
  jwtSecret: string;
  jwtExpiry: number;
  bcryptRounds: number;
  rateLimitWindow: number;
  rateLimitMax: number;
  logLevel: 'debug' | 'info' | 'warn' | 'error';
}

export const config: AppConfig = {
  port: parseInt(process.env.PORT || '3000', 10),
  jwtSecret: process.env.JWT_SECRET || 'dev-secret-change-in-production',
  jwtExpiry: parseInt(process.env.JWT_EXPIRY || '86400000', 10),
  bcryptRounds: parseInt(process.env.BCRYPT_ROUNDS || '12', 10),
  rateLimitWindow: 15 * 60 * 1000,
  rateLimitMax: 100,
  logLevel: (process.env.LOG_LEVEL as AppConfig['logLevel']) || 'info',
};

export function validateConfig(cfg: AppConfig): string[] {
  const errors: string[] = [];
  if (!cfg.jwtSecret || cfg.jwtSecret === 'dev-secret-change-in-production') {
    errors.push('JWT_SECRET must be set in production');
  }
  if (cfg.port < 1 || cfg.port > 65535) {
    errors.push('PORT must be between 1 and 65535');
  }
  if (cfg.jwtExpiry < 60000) {
    errors.push('JWT_EXPIRY must be at least 60 seconds');
  }
  return errors;
}
