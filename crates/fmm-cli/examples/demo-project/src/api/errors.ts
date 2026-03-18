import { Request, Response, NextFunction } from 'express';

export class AppError extends Error {
  constructor(
    public statusCode: number,
    message: string,
    public code?: string
  ) {
    super(message);
    this.name = 'AppError';
  }
}

export function notFound(resource: string): AppError {
  return new AppError(404, `${resource} not found`, 'NOT_FOUND');
}

export function unauthorized(message = 'Unauthorized'): AppError {
  return new AppError(401, message, 'UNAUTHORIZED');
}

export function errorHandler(err: Error, _req: Request, res: Response, _next: NextFunction): void {
  if (err instanceof AppError) {
    res.status(err.statusCode).json({ error: err.message, code: err.code });
  } else {
    res.status(500).json({ error: 'Internal server error' });
  }
}
