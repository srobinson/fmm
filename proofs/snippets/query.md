## The Navigation Query

> Describe the architecture of this project. What are the main modules, their roles, key exports, and how they depend on each other? Be specific about file paths.

This query asks an LLM to **understand an entire codebase** — the task where fmm has the highest impact. The LLM must identify modules, map dependencies, and describe the system architecture.

**Target:** 18-file TypeScript authentication app (auth, API routes, middleware, services).
