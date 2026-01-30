## What the LLM Sees: `.fmm/index.json`

```json
{
  "version": "1.0",
  "files": {
    "index.ts": {
      "exports": ["createApp"],
      "dependencies": ["./api/routes/auth", "./api/routes/users", "./config/app", "./middleware/rateLimit"],
      "loc": 30
    },
    "auth/login.ts": {
      "exports": ["login"],
      "dependencies": ["./types", "./jwt", "./password", "../services/audit", "../api/models/user"],
      "loc": 42
    },
    "auth/jwt.ts": {
      "exports": ["createToken", "verifyToken", "TokenPayload"],
      "dependencies": ["./types", "../config/app"],
      "loc": 30
    },
    "services/email.ts": {
      "exports": ["EmailMessage", "sendEmail", "sendWelcomeEmail", "sendPasswordResetEmail", "..."],
      "dependencies": [],
      "loc": 35
    }
  }
}
```

Every file's role, exports, and dependencies — without opening a single source file.
