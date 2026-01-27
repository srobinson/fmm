// ---
// file: examples/sample.ts
// exports: [SessionConfig, SessionManager, refreshToken]
// imports: [jsonwebtoken, redis]
// loc: 43
// modified: 2026-01-27
// ---

import jwt from 'jsonwebtoken'
import { RedisClient } from 'redis'

export interface SessionConfig {
  ttl: number
  secret: string
}

export class SessionManager {
  private redis: RedisClient
  private config: SessionConfig

  constructor(redis: RedisClient, config: SessionConfig) {
    this.redis = redis
    this.config = config
  }

  async createSession(userId: string): Promise<string> {
    const token = jwt.sign({ userId }, this.config.secret, {
      expiresIn: this.config.ttl,
    })
    await this.redis.set(`session:${userId}`, token, { EX: this.config.ttl })
    return token
  }

  async validateSession(token: string): Promise<boolean> {
    try {
      const decoded = jwt.verify(token, this.config.secret)
      return !!decoded
    } catch {
      return false
    }
  }

  async destroySession(userId: string): Promise<void> {
    await this.redis.del(`session:${userId}`)
  }
}

export function refreshToken(oldToken: string, secret: string): string {
  const decoded = jwt.verify(oldToken, secret)
  return jwt.sign(decoded, secret, { expiresIn: 3600 })
}
