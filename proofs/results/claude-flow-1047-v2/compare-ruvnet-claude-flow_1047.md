## fmm gh issue --compare Results

**Issue:** ruvnet/claude-flow#1047 — Statusline ADR count is hardcoded to 0/0
**Model:** sonnet | **Budget:** $5.00 | **Max turns:** 30
**Timestamp:** 2026-01-31T17:46:54.546983+00:00

| Metric | Control | FMM | Delta | Savings |
|--------|---------|-----|-------|---------|
| Input tokens | 32 | 22 | -10 | 31% |
| Output tokens | 6.9K | 8.5K | +1613 | — |
| Cache read tokens | 963.7K | 2.4M | +1.5M | — |
| Total cost | $0.72 | $1.29 | +$0.57 | -79% |
| Turns | 27 | 29 | +2 | -7% |
| Tool calls | 45 | 28 | -17 | 38% |
| Files read | 15 | 8 | -7 | 47% |
| Files changed | 1 | 1 | 0 | — |
| Duration | 182s | 248s | +67s | -37% |

### Tool Breakdown

| Tool | Control | FMM |
|------|---------|-----|
| Bash | 9 | 14 |
| Edit | 3 | 3 |
| Glob | 6 | 1 |
| Grep | 7 | 1 |
| Read | 15 | 8 |
| Task | 1 | 0 |
| TodoWrite | 4 | 0 |
| Write | 0 | 1 |

**Verdict:** fmm did not reduce token usage in this run.

### Diffs

<details>
<summary>Control diff</summary>

```diff
diff --git a/v3/@claude-flow/cli/src/commands/hooks.ts b/v3/@claude-flow/cli/src/commands/hooks.ts
index 1972647..6b90df0 100644
--- a/v3/@claude-flow/cli/src/commands/hooks.ts
+++ b/v3/@claude-flow/cli/src/commands/hooks.ts
@@ -3493,12 +3493,81 @@ const statuslineCommand: Command = {
       return { name, gitBranch, modelName };
     }
 
+    // Get ADR (Architecture Decision Records) status
+    function getADRStatus() {
+      let compliance = 0;
+      let totalChecks = 0;
+      let compliantChecks = 0;
+
+      // Check adr-compliance.json for REAL compliance data
+      const compliancePath = path.join(process.cwd(), '.claude-flow', 'metrics', 'adr-compliance.json');
+      if (fs.existsSync(compliancePath)) {
+        try {
+          const data = JSON.parse(fs.readFileSync(compliancePath, 'utf-8'));
+          compliance = data.compliance || 0;
+          const checks = data.checks || {};
+          totalChecks = Object.keys(checks).length;
+          compliantChecks = Object.values(checks).filter((c: any) => c.compliant).length;
+          return { count: totalChecks, implemented: compliantChecks, compliance };
+        } catch {
+          // Fall through to file-based detection
+        }
+      }
+
+      // Fallback: count ADR files directly
+      const adrPaths = [
+        path.join(process.cwd(), 'docs', 'adrs'),
+        path.join(process.cwd(), 'docs', 'adr'),
+        path.join(process.cwd(), 'adr'),
+        path.join(process.cwd(), 'ADR'),
+        path.join(process.cwd(), '.claude-flow', 'adrs'),
+        path.join(process.cwd(), 'v3', 'implementation', 'adrs'),
+        path.join(process.cwd(), 'implementation', 'adrs'),
+      ];
+
+      let count = 0;
+      let implemented = 0;
+
+      for (const adrPath of adrPaths) {
+        if (fs.existsSync(adrPath)) {
+          try {
+            const files = fs.readdirSync(adrPath).filter((f: string) =>
+              f.endsWith('.md') && (f.startsWith('ADR-') || f.startsWith('adr-') || /^\d{4}-/.test(f))
+            );
+            count = files.length;
+
+            for (const file of files) {
+              try {
+                const content = fs.readFileSync(path.join(adrPath, file), 'utf-8');
+                // Support both plain and markdown bold status headers
+                if (content.includes('Status: Implemented') || content.includes('status: implemented') ||
+                    content.includes('Status: Accepted') || content.includes('status: accepted') ||
+                    content.includes('**Status**: Implemented') || content.includes('**Status**: Accepted') ||
+                    content.includes('**status**: implemented') || content.includes('**status**: accepted')) {
+                  implemented++;
+                }
+              } catch {
+                // Skip unreadable files
+              }
+            }
+            break;
+          } catch {
+            // Ignore
+          }
+        }
+      }
+
+      compliance = count > 0 ? Math.floor((implemented / count) * 100) : 0;
+      return { count, implemented, compliance };
+    }
+
     // Collect all status
     const progress = getV3Progress();
     const security = getSecurityStatus();
     const swarm = getSwarmStatus();
     const system = getSystemMetrics();
     const user = getUserInfo();
+    const adrs = getADRStatus();
 
     const statusData = {
       user,
@@ -3506,6 +3575,7 @@ const statuslineCommand: Command = {
       security,
       swarm,
       system,
+      adrs,
       timestamp: new Date().toISOString()
     };
 
@@ -3708,8 +3778,13 @@ const statuslineCommand: Command = {
       `${c.brightPurple}🧠 ${String(system.intelligencePct).padStart(3)}%${c.reset}`;
 
     const dddColor = progress.dddProgress >= 50 ? c.brightGreen : progress.dddProgress > 0 ? c.yellow : c.red;
+    // ADR display: show compliance % if from real data, otherwise show count
+    const adrColor = adrs.count > 0 ? (adrs.implemented === adrs.count ? c.brightGreen : c.yellow) : c.dim;

... (9 more lines)
```
</details>

<details>
<summary>FMM diff</summary>

```diff
diff --git a/v3/@claude-flow/cli/src/commands/hooks.ts b/v3/@claude-flow/cli/src/commands/hooks.ts
index 1972647..7ee9b7f 100644
--- a/v3/@claude-flow/cli/src/commands/hooks.ts
+++ b/v3/@claude-flow/cli/src/commands/hooks.ts
@@ -3469,6 +3469,71 @@ const statuslineCommand: Command = {
       return { memoryMB, contextPct, intelligencePct, subAgents };
     }
 
+    // Get ADR (Architecture Decision Records) status
+    function getADRStatus() {
+      let count = 0;
+      let implemented = 0;
+      let compliance = 0;
+
+      // Check adr-compliance.json for REAL compliance data first
+      const compliancePath = path.join(process.cwd(), '.claude-flow', 'metrics', 'adr-compliance.json');
+      if (fs.existsSync(compliancePath)) {
+        try {
+          const data = JSON.parse(fs.readFileSync(compliancePath, 'utf-8'));
+          compliance = data.compliance || 0;
+          const checks = data.checks || {};
+          count = Object.keys(checks).length;
+          implemented = Object.values(checks).filter((c: any) => c.compliant).length;
+          return { count, implemented, compliance };
+        } catch {
+          // Fall through to file-based detection
+        }
+      }
+
+      // Fallback: count ADR files directly
+      const adrPaths = [
+        path.join(process.cwd(), 'docs', 'adrs'),
+        path.join(process.cwd(), 'docs', 'adr'),
+        path.join(process.cwd(), 'adr'),
+        path.join(process.cwd(), 'ADR'),
+        path.join(process.cwd(), '.claude-flow', 'adrs'),
+        path.join(process.cwd(), 'v3', 'implementation', 'adrs'),
+        path.join(process.cwd(), 'implementation', 'adrs'),
+      ];
+
+      for (const adrPath of adrPaths) {
+        if (fs.existsSync(adrPath)) {
+          try {
+            const files = fs.readdirSync(adrPath).filter(f =>
+              f.endsWith('.md') && (f.startsWith('ADR-') || f.startsWith('adr-') || /^\d{4}-/.test(f))
+            );
+            count = files.length;
+
+            for (const file of files) {
+              try {
+                const content = fs.readFileSync(path.join(adrPath, file), 'utf-8');
+                // Support both plain and markdown bold status
+                if (content.includes('Status: Implemented') || content.includes('status: implemented') ||
+                    content.includes('Status: Accepted') || content.includes('status: accepted') ||
+                    content.includes('**Status**: Implemented') || content.includes('**Status**: Accepted') ||
+                    content.includes('**status**: implemented') || content.includes('**status**: accepted')) {
+                  implemented++;
+                }
+              } catch {
+                // Skip unreadable files
+              }
+            }
+            break;
+          } catch {
+            // Ignore
+          }
+        }
+      }
+
+      compliance = count > 0 ? Math.floor((implemented / count) * 100) : 0;
+      return { count, implemented, compliance };
+    }
+
     // Get user info
     function getUserInfo() {
       let name = 'user';
@@ -3499,6 +3564,7 @@ const statuslineCommand: Command = {
     const swarm = getSwarmStatus();
     const system = getSystemMetrics();
     const user = getUserInfo();
+    const adrs = getADRStatus();
 
     const statusData = {
       user,
@@ -3506,6 +3572,7 @@ const statuslineCommand: Command = {
       security,
       swarm,
       system,
+      adrs,
       timestamp: new Date().toISOString()
     };
 
@@ -3708,8 +3775,15 @@ const statuslineCommand: Command = {
       `${c.brightPurple}🧠 ${String(system.intelligencePct).padStart(3)}%${c.reset}`;
 
     const dddColor = progress.dddProgress >= 50 ? c.brightGreen : progress.dddProgress > 0 ? c.yellow : c.red;
+    const adrColor = adrs.count > 0 ? (adrs.implemented === adrs.count ? c.brightGreen : c.yellow) : c.dim;
+
+    // Show ADR compliance % if from real data, otherwise show count
+    const adrDisplay = adrs.compliance > 0

... (9 more lines)
```
</details>
