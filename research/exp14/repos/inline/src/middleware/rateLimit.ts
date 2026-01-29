// --- FMM ---
// fmm: v0.2
// file: middleware/rateLimit.ts
// exports: [authRateLimiter, clearRateLimitStore, rateLimiter]
// dependencies: [./auth]
// loc: 47
// modified: 2026-01-29
// ---

import { AuthenticatedRequest, Response } from './auth';

interface RateLimitEntry {
  count: number;
  resetAt: number;
}

const store = new Map<string, RateLimitEntry>();

type NextFunction = () => void;

export function rateLimiter(maxRequests: number, windowMs: number) {
  return (req: AuthenticatedRequest, res: Response, next: NextFunction): void => {
    const key = getClientKey(req);
    const now = Date.now();
    const entry = store.get(key);

    if (!entry || entry.resetAt < now) {
      store.set(key, { count: 1, resetAt: now + windowMs });
      next();
      return;
    }

    if (entry.count >= maxRequests) {
      res.status(429).json({
        error: 'Too many requests',
        retryAfter: Math.ceil((entry.resetAt - now) / 1000),
      });
      return;
    }

    entry.count++;
    next();
  };
}

export function authRateLimiter() {
  return rateLimiter(5, 15 * 60 * 1000);
}

function getClientKey(req: AuthenticatedRequest): string {
  return req.headers['x-forwarded-for'] || req.headers['x-real-ip'] || 'unknown';
}

export function clearRateLimitStore(): void {
  store.clear();
}
