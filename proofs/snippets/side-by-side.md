## Without fmm vs With fmm

```
Without fmm                          With fmm
─────────────                        ────────
1. ls -la                            1. Read .fmm/index.json     <-- manifest first
2. Glob(**/*.ts)                     2. Read package.json
3. Glob(**/*.tsx)                    3. ls (confirm structure)
4. tree src/                         4. Read src/index.ts
5. Read tsconfig.json                5. Read src/config/app.ts
6. Read src/index.ts                 6. Read src/auth/types.ts
7. Read src/config/app.ts            7. Read src/api/models/user.ts
8. Read src/api/routes/auth.ts       8. Read src/api/routes/auth.ts
9. Read src/api/routes/users.ts      9. Read src/auth/login.ts
10. Read src/api/controllers/...     10. Read src/middleware/auth.ts
11. Read src/api/models/user.ts      11. ... done (16 tool calls)
12. Read src/api/models/session.ts
13. Read src/auth/types.ts
14. Read src/auth/login.ts
15. Read src/auth/signup.ts
16. Read src/auth/jwt.ts
17. Read src/auth/password.ts
18. Read src/middleware/auth.ts
19. Read src/middleware/rateLimit.ts
20. Read src/services/audit.ts
21. Read src/services/email.ts
22. Read src/utils/id.ts
23. Read src/utils/validation.ts
24. ... done (25 tool calls)

25 tool calls, 19 files read            16 tool calls, 9 source files read
318,589 tokens                          220,895 tokens
93 seconds                              67 seconds
```

Same architectural understanding. **36% fewer tool calls. 53% fewer source reads. 31% fewer tokens.**
