# Demo Project Walkthrough

This is a small Express.js web application with authentication, database access, and API routes. All 8 source files have pre-generated `.fmm` sidecars.

## Project structure

```
src/
  index.ts              Entry point — wires up Express, DB, routes
  config/index.ts       App configuration from environment variables
  db/client.ts          PostgreSQL connection pool wrapper
  db/users.ts           User CRUD operations
  auth/session.ts       JWT session creation and validation
  auth/middleware.ts     Express auth middleware
  api/routes.ts         Login, register, and user routes
  api/errors.ts         Error types and Express error handler
```

## Try it with fmm

### 1. Find where a symbol is defined

```bash
$ fmm search --export createSession
✓ 1 file(s) found:

src/auth/session.ts
  exports: SessionPayload, createSession, validateSession, destroySession
  imports: jsonwebtoken
  loc: 32
```

### 2. Find what depends on the auth module

```bash
$ fmm search --depends-on src/auth/session
✓ 1 file(s) found:

src/auth/middleware.ts
  exports: AuthenticatedRequest, requireAuth
  imports: express
  loc: 27
```

### 3. Find all files that import express

```bash
$ fmm search --imports express
✓ 4 file(s) found:

src/api/errors.ts
  exports: AppError, notFound, unauthorized, errorHandler
  loc: 29

src/api/routes.ts
  exports: createRouter
  loc: 36

src/auth/middleware.ts
  exports: AuthenticatedRequest, requireAuth
  loc: 27

src/index.ts
  exports:
  loc: 21
```

### 4. Find large files

```bash
$ fmm search --loc ">30"
✓ 2 file(s) found:

src/api/routes.ts
  loc: 36

src/auth/session.ts
  loc: 32
```

## What an LLM sees

Instead of reading 214 lines of source code across 8 files, an LLM reads 8 sidecars totaling ~56 lines of YAML. It knows every export, import, and dependency — enough to navigate to exactly the right file.

**Without fmm:** ~4,000 tokens to read all source files
**With fmm:** ~200 tokens to read all sidecars
