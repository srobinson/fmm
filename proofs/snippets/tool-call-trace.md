## Before/After Tool Call Trace

### Control (no fmm) — Architecture Overview

```
 1. Bash(pwd)
 2. Bash(ls -la)
 3. Glob(**/*.json)
 4. Glob(**/*.ts)
 5. Glob(**/*.tsx)
 6. Bash(tree -d src/)
 7. Read(src/index.ts)                    # starts reading every file
 8. Read(tsconfig.json)
 9. Read(src/config/app.ts)
10. Read(src/api/routes/auth.ts)
11. Read(src/api/routes/users.ts)
12. Read(src/api/controllers/authController.ts)
13. Read(src/api/models/user.ts)
14. Read(src/api/models/session.ts)
15. Read(src/auth/types.ts)
16. Read(src/auth/login.ts)
17. Read(src/auth/signup.ts)
18. Read(src/auth/jwt.ts)
19. Read(src/auth/password.ts)
20. Read(src/middleware/auth.ts)
21. Read(src/middleware/rateLimit.ts)
22. Read(src/services/audit.ts)
23. Read(src/services/email.ts)
24. Read(src/utils/id.ts)
25. Read(src/utils/validation.ts)
```

**25 tool calls. Read all 19 source files.** No map — just brute force.

### Treatment (with fmm) — Same Query

```
 1. Read(.fmm/index.json)                # manifest first!
 2. Read(package.json)
 3. Bash(pwd)
 4. Bash(ls -la)
 5. Bash(find . -name "*.json")
 6. Bash(ls -la src/)
 7. Read(src/.fmm/index.json)            # confirms manifest location
 8. Read(src/index.ts)                   # selective reads from here
 9. Read(tsconfig.json)
10. Bash(tree -L 3 src/)
11. Read(src/config/app.ts)
12. Read(src/auth/types.ts)
13. Read(src/api/models/user.ts)
14. Read(src/api/routes/auth.ts)
15. Read(src/auth/login.ts)
16. Read(src/middleware/auth.ts)
```

**16 tool calls. Read 9 source files.** Manifest told it what each file does — only opened the key ones.
