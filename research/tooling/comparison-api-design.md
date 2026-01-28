# FMM Comparison API Design

**Purpose:** Automated benchmarking service to demonstrate fmm value proposition. User provides repo, system proves token savings.

**Status:** Architecture design

---

## Executive Summary

A comparison API that:
1. Accepts a GitHub repo URL
2. Creates two variants (control, fmm-enhanced)
3. Runs standardized test prompts against both
4. Returns quantified metrics proving fmm's value

The design prioritizes local-first operation with cloud extensibility.

---

## Architecture Overview

```
+-----------------------------------------------------------------------------+
|                           FMM COMPARISON SYSTEM                              |
+-----------------------------------------------------------------------------+
|                                                                              |
|   +----------+     +--------------+     +-----------------------------+     |
|   |   CLI    |---->|  Orchestrator|---->|        Job Queue            |     |
|   | (fmm cmp)|     |              |     |   (SQLite / Redis)          |     |
|   +----------+     +------+-------+     +-------------+---------------+     |
|                           |                           |                      |
|   +----------+            |                           |                      |
|   |  REST    |------------+                           |                      |
|   |  API     |                                        v                      |
|   +----------+                          +-----------------------------+     |
|                                         |        Worker Pool          |     |
|                                         |  +-------+ +-------+        |     |
|                                         |  |Worker | |Worker | ...    |     |
|                                         |  |  1    | |  2    |        |     |
|                                         |  +---+---+ +---+---+        |     |
|                                         +------+---------+------------+     |
|                                                |         |                  |
|                    +---------------------------+---------+----------+       |
|                    |              Sandbox Manager                    |       |
|                    |  +------------+      +------------+            |       |
|                    |  | Sandbox A  |      | Sandbox B  |            |       |
|                    |  | (control)  |      |   (fmm)    |            |       |
|                    |  |            |      |            |            |       |
|                    |  | +--------+ |      | +--------+ |            |       |
|                    |  | | Clone  | |      | | Clone  | |            |       |
|                    |  | | of repo| |      | |+manifest| |            |       |
|                    |  | +--------+ |      | +--------+ |            |       |
|                    |  +------------+      +------------+            |       |
|                    +------------------------------------------------+       |
|                                                                             |
|                    +------------------------------------------------+       |
|                    |              Test Runner                        |       |
|                    |  - Claude API integration                      |       |
|                    |  - Instrumented tool calls                     |       |
|                    |  - Token/accuracy measurement                  |       |
|                    +------------------------------------------------+       |
|                                                                             |
|                    +------------------------------------------------+       |
|                    |              Results Store                      |       |
|                    |  - SQLite (local) / PostgreSQL (cloud)         |       |
|                    |  - Comparison reports                          |       |
|                    |  - Historical data                             |       |
|                    +------------------------------------------------+       |
|                                                                             |
+-----------------------------------------------------------------------------+
```

---

## Interface Design

### Option Analysis: CLI vs REST vs Both

| Approach | Pros | Cons | Recommendation |
|----------|------|------|----------------|
| **CLI only** | Simple, local-first, no server | No async jobs, no web UI | Good for v1 |
| **REST only** | Async, scalable, integrations | Requires server, complexity | Cloud-focused |
| **Both** | Best of both worlds | More code to maintain | Target for v2 |

**Decision:** Start with CLI, add REST as a thin wrapper over the same core.

### CLI Interface

```bash
# Basic comparison
fmm compare https://github.com/pmndrs/zustand

# With options
fmm compare https://github.com/pmndrs/zustand \
  --branch main \
  --src-path src/ \
  --tasks ./custom-tasks.json \
  --runs 3 \
  --output ./results/ \
  --format json|markdown|html

# Quick mode (fewer tests, faster results)
fmm compare --quick https://github.com/owner/repo

# Resume interrupted comparison
fmm compare --resume <job-id>

# List past comparisons
fmm compare --list

# View specific result
fmm compare --show <job-id>
```

### REST API

```yaml
openapi: 3.0.0
info:
  title: FMM Comparison API
  version: 1.0.0

paths:
  /api/v1/compare:
    post:
      summary: Start a new comparison job
      requestBody:
        content:
          application/json:
            schema:
              type: object
              required:
                - repo_url
              properties:
                repo_url:
                  type: string
                  example: "https://github.com/pmndrs/zustand"
                branch:
                  type: string
                  default: "main"
                src_path:
                  type: string
                  default: "src/"
                task_set:
                  type: string
                  enum: [standard, quick, comprehensive]
                  default: "standard"
                runs_per_task:
                  type: integer
                  minimum: 1
                  maximum: 5
                  default: 3
                webhook_url:
                  type: string
                  description: "URL to POST results when complete"
      responses:
        202:
          description: Job accepted
          content:
            application/json:
              schema:
                type: object
                properties:
                  job_id:
                    type: string
                  status_url:
                    type: string
                  estimated_time_seconds:
                    type: integer

  /api/v1/compare/{job_id}:
    get:
      summary: Get comparison job status and results
      parameters:
        - name: job_id
          in: path
          required: true
          schema:
            type: string
      responses:
        200:
          description: Job status
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ComparisonResult'

  /api/v1/compare/{job_id}/cancel:
    post:
      summary: Cancel a running job

components:
  schemas:
    ComparisonResult:
      type: object
      properties:
        job_id:
          type: string
        status:
          type: string
          enum: [queued, cloning, generating, testing, complete, failed, cancelled]
        repo_url:
          type: string
        created_at:
          type: string
          format: date-time
        completed_at:
          type: string
          format: date-time
        metrics:
          $ref: '#/components/schemas/ComparisonMetrics'
        tasks:
          type: array
          items:
            $ref: '#/components/schemas/TaskResult'
        error:
          type: string

    ComparisonMetrics:
      type: object
      properties:
        control:
          $ref: '#/components/schemas/VariantMetrics'
        fmm:
          $ref: '#/components/schemas/VariantMetrics'
        savings:
          type: object
          properties:
            lines_read_percent:
              type: number
            tokens_percent:
              type: number
            files_accessed_percent:
              type: number
            time_percent:
              type: number

    VariantMetrics:
      type: object
      properties:
        total_lines_read:
          type: integer
        total_tokens:
          type: integer
        total_files_accessed:
          type: integer
        total_read_calls:
          type: integer
        avg_time_ms:
          type: number
        accuracy:
          type: number

    TaskResult:
      type: object
      properties:
        task_id:
          type: string
        scenario:
          type: string
        control:
          $ref: '#/components/schemas/RunResult'
        fmm:
          $ref: '#/components/schemas/RunResult'

    RunResult:
      type: object
      properties:
        lines_read:
          type: integer
        files_accessed:
          type: array
          items:
            type: string
        read_calls:
          type: integer
        tokens:
          type: integer
        time_ms:
          type: integer
        accuracy:
          type: number
        correct:
          type: boolean
```

---

## Job Queue Design

### Requirements

- Async processing (clone + tests = minutes)
- Job persistence (survive restarts)
- Status tracking
- Result storage
- Rate limiting
- Priority queuing (optional)

### Local Implementation (SQLite + Threads)

```sql
-- schema.sql
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    repo_url TEXT NOT NULL,
    branch TEXT DEFAULT 'main',
    src_path TEXT DEFAULT 'src/',
    task_set TEXT DEFAULT 'standard',
    runs_per_task INTEGER DEFAULT 3,
    status TEXT DEFAULT 'queued',
    progress_percent INTEGER DEFAULT 0,
    current_step TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME,
    error_message TEXT,
    webhook_url TEXT
);

CREATE TABLE job_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT REFERENCES jobs(id),
    task_id TEXT NOT NULL,
    variant TEXT NOT NULL, -- 'control' or 'fmm'
    run_number INTEGER NOT NULL,
    lines_read INTEGER,
    files_accessed TEXT, -- JSON array
    read_calls INTEGER,
    tokens INTEGER,
    time_ms INTEGER,
    accuracy REAL,
    correct BOOLEAN,
    raw_output TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE repo_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_url TEXT NOT NULL,
    branch TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    cache_path TEXT NOT NULL,
    manifest_path TEXT,
    file_count INTEGER,
    total_loc INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_accessed DATETIME,
    UNIQUE(repo_url, branch, commit_sha)
);

CREATE INDEX idx_jobs_status ON jobs(status);
CREATE INDEX idx_job_results_job_id ON job_results(job_id);
CREATE INDEX idx_repo_cache_url ON repo_cache(repo_url, branch);
```

### Worker Architecture (Pseudocode)

```rust
struct WorkerPool {
    workers: Vec<Worker>,
    job_queue: Arc<Mutex<JobQueue>>,
    db: Database,
}

struct Worker {
    id: usize,
    sandbox_dir: PathBuf,
}

impl Worker {
    async fn run(&self) {
        loop {
            let job = self.job_queue.lock().await.pop();
            if let Some(job) = job {
                self.process_job(job).await;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn process_job(&self, job: Job) {
        // 1. Clone repo (or use cache)
        self.update_status(&job, "cloning");
        let control_path = self.clone_repo(&job, "control").await?;
        let fmm_path = self.clone_repo(&job, "fmm").await?;

        // 2. Generate fmm manifest for fmm variant
        self.update_status(&job, "generating");
        self.generate_manifest(&fmm_path).await?;

        // 3. Run tests
        self.update_status(&job, "testing");
        let tasks = self.load_tasks(&job.task_set);

        for task in tasks {
            for run in 1..=job.runs_per_task {
                let control_result = self.run_task(&task, &control_path, "control").await?;
                self.save_result(&job, &task, "control", run, control_result).await?;

                let fmm_result = self.run_task(&task, &fmm_path, "fmm").await?;
                self.save_result(&job, &task, "fmm", run, fmm_result).await?;
            }
        }

        // 4. Aggregate and complete
        self.update_status(&job, "complete");
        if let Some(webhook) = &job.webhook_url {
            self.send_webhook(webhook, &job).await;
        }
    }
}
```

### Cloud Extension (Redis + Workers)

For cloud deployment, swap SQLite for:
- **Redis** for job queue (with persistence)
- **PostgreSQL** for results storage
- **Kubernetes Jobs** or **Cloud Run Jobs** for workers

```yaml
# docker-compose.yml for cloud-ready local dev
version: '3.8'

services:
  api:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://user:pass@db:5432/fmm
      - REDIS_URL=redis://redis:6379
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    depends_on:
      - db
      - redis

  worker:
    build: .
    command: fmm-worker
    environment:
      - DATABASE_URL=postgres://user:pass@db:5432/fmm
      - REDIS_URL=redis://redis:6379
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    deploy:
      replicas: 2

  db:
    image: postgres:15
    volumes:
      - pgdata:/var/lib/postgresql/data

  redis:
    image: redis:7
    volumes:
      - redisdata:/data

volumes:
  pgdata:
  redisdata:
```

---

## Sandboxing and Isolation

### Requirements

- Each comparison runs in isolation
- Prevent cross-contamination between control/fmm
- Limit resource usage (disk, time, API calls)
- Clean up after completion

### Implementation Options

| Approach | Isolation | Complexity | Local-First |
|----------|-----------|------------|-------------|
| **Directory-based** | Medium | Low | Yes |
| **Docker containers** | High | Medium | Yes |
| **Firecracker microVMs** | Very High | High | No |
| **Nsjail** | High | Medium | Linux only |

**Decision:** Directory-based for v1, Docker optional for v2.

### Directory-Based Sandbox

```rust
struct Sandbox {
    root: PathBuf,           // /tmp/fmm-compare-{job_id}/
    control_dir: PathBuf,    // /tmp/fmm-compare-{job_id}/control/
    fmm_dir: PathBuf,        // /tmp/fmm-compare-{job_id}/fmm/
    max_size_mb: u64,
    max_files: u64,
    created_at: Instant,
    ttl: Duration,
}

impl Sandbox {
    fn new(job_id: &str) -> Self {
        let root = std::env::temp_dir().join(format!("fmm-compare-{}", job_id));
        fs::create_dir_all(&root)?;

        Self {
            control_dir: root.join("control"),
            fmm_dir: root.join("fmm"),
            root,
            max_size_mb: 500,       // 500MB max per sandbox
            max_files: 10_000,      // 10K files max
            created_at: Instant::now(),
            ttl: Duration::from_secs(3600), // 1 hour TTL
        }
    }

    fn check_limits(&self) -> Result<(), SandboxError> {
        let size = dir_size(&self.root)?;
        if size > self.max_size_mb * 1_000_000 {
            return Err(SandboxError::SizeExceeded);
        }

        let files = count_files(&self.root)?;
        if files > self.max_files {
            return Err(SandboxError::FileLimitExceeded);
        }

        if self.created_at.elapsed() > self.ttl {
            return Err(SandboxError::Expired);
        }

        Ok(())
    }

    fn cleanup(&self) {
        fs::remove_dir_all(&self.root).ok();
    }
}
```

### Resource Limits

```rust
struct ResourceLimits {
    // Repo cloning
    max_repo_size_mb: u64,        // 500 MB
    clone_timeout_secs: u64,      // 300 seconds

    // Manifest generation
    max_files_to_parse: u64,      // 5000 files
    parse_timeout_secs: u64,      // 120 seconds

    // Test execution
    max_api_calls_per_task: u64,  // 50 calls
    max_tokens_per_task: u64,     // 100K tokens
    task_timeout_secs: u64,       // 180 seconds

    // Total job
    max_total_api_cost: f64,      // $5.00
    job_timeout_secs: u64,        // 1800 seconds (30 min)
}
```

---

## Result Storage and Caching

### Caching Strategy

```
+----------------------------------------------------------------+
|                        CACHING LAYERS                           |
+----------------------------------------------------------------+
|                                                                  |
|  1. REPO CACHE                                                  |
|     Key: (repo_url, branch, commit_sha)                         |
|     Value: cloned directory path                                |
|     TTL: 24 hours (configurable)                                |
|     Benefit: Skip clone on repeated comparisons                 |
|                                                                  |
|  2. MANIFEST CACHE                                              |
|     Key: (repo_url, commit_sha)                                 |
|     Value: generated .fmm/manifest.json                         |
|     TTL: Until repo changes                                     |
|     Benefit: Skip manifest generation                           |
|                                                                  |
|  3. RESULT CACHE                                                |
|     Key: (repo_url, commit_sha, task_id, variant)               |
|     Value: test results                                         |
|     TTL: 7 days                                                 |
|     Benefit: Return cached results for identical comparisons    |
|                                                                  |
+----------------------------------------------------------------+
```

### Cache Implementation

```rust
struct CacheManager {
    cache_dir: PathBuf,           // ~/.fmm/cache/
    db: Database,
    max_cache_size_gb: u64,       // 10 GB default
}

impl CacheManager {
    fn get_repo(&self, url: &str, branch: &str) -> Option<CachedRepo> {
        let record = self.db.query(
            "SELECT * FROM repo_cache WHERE repo_url = ? AND branch = ?
             ORDER BY created_at DESC LIMIT 1",
            [url, branch]
        )?;

        if let Some(record) = record {
            if Path::new(&record.cache_path).exists() {
                self.db.execute(
                    "UPDATE repo_cache SET last_accessed = CURRENT_TIMESTAMP WHERE id = ?",
                    [record.id]
                );
                return Some(record);
            }
        }
        None
    }

    fn store_repo(&self, url: &str, branch: &str, commit_sha: &str, path: &Path) {
        let cache_path = self.cache_dir
            .join("repos")
            .join(sanitize_url(url))
            .join(commit_sha);

        fs::rename(path, &cache_path)?;

        self.db.execute(
            "INSERT INTO repo_cache (repo_url, branch, commit_sha, cache_path) VALUES (?, ?, ?, ?)",
            [url, branch, commit_sha, cache_path.to_str().unwrap()]
        );

        self.evict_if_needed();
    }

    fn evict_if_needed(&self) {
        let current_size = self.calculate_cache_size();
        if current_size > self.max_cache_size_gb * 1_000_000_000 {
            let entries = self.db.query(
                "SELECT * FROM repo_cache ORDER BY last_accessed ASC"
            );

            for entry in entries {
                fs::remove_dir_all(&entry.cache_path).ok();
                self.db.execute("DELETE FROM repo_cache WHERE id = ?", [entry.id]);

                if self.calculate_cache_size() < self.max_cache_size_gb * 1_000_000_000 * 0.8 {
                    break;
                }
            }
        }
    }
}
```

---

## Cost Management

### API Call Economics

```
Per comparison (assuming medium repo, 5 tasks, 3 runs each):

Control variant:
  - 5 tasks x 3 runs x ~15,000 input tokens = 225,000 tokens
  - 5 tasks x 3 runs x ~2,000 output tokens = 30,000 tokens
  - Cost at Claude pricing: ~$3.60 input + ~$2.25 output = ~$5.85

FMM variant:
  - 5 tasks x 3 runs x ~3,000 input tokens = 45,000 tokens
  - 5 tasks x 3 runs x ~2,000 output tokens = 30,000 tokens
  - Cost at Claude pricing: ~$0.68 input + ~$2.25 output = ~$2.93

Total per comparison: ~$8.78

With caching (repeat comparison, same commit):
  - Use cached results: $0
```

### Cost Controls

```rust
struct CostManager {
    max_cost_per_job: f64,        // $10.00
    daily_budget_per_user: f64,   // $50.00
    monthly_budget_per_user: f64, // $500.00
    daily_system_budget: f64,     // $1000.00
    db: Database,
}

impl CostManager {
    fn can_start_job(&self, user_id: &str, estimated_cost: f64) -> Result<bool, CostError> {
        let user_today = self.get_user_spend_today(user_id)?;
        if user_today + estimated_cost > self.daily_budget_per_user {
            return Err(CostError::DailyLimitExceeded);
        }

        let system_today = self.get_system_spend_today()?;
        if system_today + estimated_cost > self.daily_system_budget {
            return Err(CostError::SystemBudgetExceeded);
        }

        Ok(true)
    }

    fn estimate_cost(&self, repo_stats: &RepoStats, task_count: usize, runs: usize) -> f64 {
        let control_tokens = repo_stats.total_loc as f64 * 0.3 * task_count as f64 * runs as f64;
        let fmm_tokens = repo_stats.manifest_size as f64 * 2.0 * task_count as f64 * runs as f64;
        let output_tokens = 2000.0 * task_count as f64 * runs as f64 * 2.0;

        let input_cost = (control_tokens + fmm_tokens) / 1_000_000.0 * 15.0;
        let output_cost = output_tokens / 1_000_000.0 * 75.0;

        input_cost + output_cost
    }
}
```

### Cost Optimization Strategies

1. **Result caching**: Same repo + commit + task = reuse results
2. **Quick mode**: Fewer tasks, 1 run each (for estimates)
3. **Smart task selection**: Skip tasks where fmm clearly wins
4. **Batch processing**: Queue jobs, run during off-peak
5. **Token-limited runs**: Stop task if exceeding estimate

---

## Test Runner Integration

### Claude API Wrapper with Instrumentation

```rust
struct InstrumentedClient {
    client: AnthropicClient,
    metrics: Arc<Mutex<RunMetrics>>,
}

struct RunMetrics {
    read_calls: u32,
    files_accessed: HashSet<String>,
    lines_read: u64,
    input_tokens: u64,
    output_tokens: u64,
    start_time: Instant,
}

impl InstrumentedClient {
    async fn run_conversation(&self, task: &Task, repo_path: &Path, variant: &str) -> RunResult {
        let system_prompt = self.build_system_prompt(repo_path, variant);
        let mut messages = vec![];
        let mut tool_results = vec![];

        messages.push(Message::user(&task.prompt));

        loop {
            let response = self.client.messages_create(MessagesRequest {
                model: "claude-opus-4-5-20251101",
                system: &system_prompt,
                messages: &messages,
                tools: self.get_tools(repo_path),
                max_tokens: 4096,
                temperature: 0.0,
            }).await?;

            let mut metrics = self.metrics.lock().await;
            metrics.input_tokens += response.usage.input_tokens;
            metrics.output_tokens += response.usage.output_tokens;

            let mut has_tool_calls = false;
            for content in &response.content {
                if let Content::ToolUse(tool_use) = content {
                    has_tool_calls = true;
                    let result = self.execute_tool(&tool_use, repo_path, &mut metrics).await?;
                    tool_results.push(result);
                }
            }

            if !has_tool_calls || response.stop_reason == "end_turn" {
                let final_text = response.content.iter()
                    .filter_map(|c| if let Content::Text(t) = c { Some(t.as_str()) } else { None })
                    .collect::<Vec<_>>()
                    .join("\n");

                return RunResult {
                    lines_read: metrics.lines_read,
                    files_accessed: metrics.files_accessed.iter().cloned().collect(),
                    read_calls: metrics.read_calls,
                    tokens: metrics.input_tokens + metrics.output_tokens,
                    time_ms: metrics.start_time.elapsed().as_millis() as u64,
                    output: final_text,
                };
            }

            messages.push(Message::assistant(&response.content));
            messages.push(Message::user_tool_results(&tool_results));
            tool_results.clear();
        }
    }

    async fn execute_tool(&self, tool: &ToolUse, repo_path: &Path, metrics: &mut RunMetrics) -> ToolResult {
        match tool.name.as_str() {
            "read" => {
                let path = tool.input.get("file_path").unwrap().as_str().unwrap();
                let full_path = repo_path.join(path);

                metrics.read_calls += 1;
                metrics.files_accessed.insert(path.to_string());

                let content = fs::read_to_string(&full_path)?;
                let lines = content.lines().count() as u64;
                metrics.lines_read += lines;

                ToolResult::success(&content)
            }
            "grep" => { /* Similar instrumentation */ }
            "glob" => { /* Similar instrumentation */ }
            _ => ToolResult::error("Unknown tool"),
        }
    }

    fn build_system_prompt(&self, repo_path: &Path, variant: &str) -> String {
        match variant {
            "control" => format!(
                "You are a code navigation assistant. Answer questions about the codebase.\n\
                 You have access to: grep (search), read (view files), glob (find files).\n\
                 The codebase is located at: {}",
                repo_path.display()
            ),
            "fmm" => format!(
                "You are a code navigation assistant. Answer questions about the codebase.\n\
                 You have access to: grep (search), read (view files), glob (find files).\n\n\
                 IMPORTANT: A manifest file exists at .fmm/manifest.json containing:\n\
                 - All files with their exports, imports, and dependencies\n\
                 - File metadata (lines of code, last modified)\n\
                 Query this manifest FIRST before reading individual files.\n\n\
                 The codebase is located at: {}",
                repo_path.display()
            ),
            _ => panic!("Unknown variant"),
        }
    }
}
```

---

## Standard Test Tasks

### Task Set: Standard (5 tasks)

```json
{
  "name": "standard",
  "description": "Standard benchmark task set for fmm comparison",
  "tasks": [
    {
      "id": "find-export",
      "scenario": "find_export",
      "prompt_template": "Which file exports the '{{symbol}}' function/type?",
      "symbol_selection": "auto",
      "expected_format": "file_path",
      "scoring": "exact_match"
    },
    {
      "id": "reverse-dependency",
      "scenario": "trace_import",
      "prompt_template": "What files import from '{{module}}'?",
      "module_selection": "auto",
      "expected_format": "file_list",
      "scoring": "f1_score"
    },
    {
      "id": "find-implementation",
      "scenario": "find_export",
      "prompt_template": "Find the {{module_name}} module and tell me what it exports",
      "module_selection": "auto",
      "expected_format": "exports_list",
      "scoring": "f1_score"
    },
    {
      "id": "trace-dependency",
      "scenario": "dependency_chain",
      "prompt_template": "The {{module_a}} - what other modules does it depend on?",
      "selection": "auto",
      "expected_format": "dependency_list",
      "scoring": "f1_score"
    },
    {
      "id": "architecture-overview",
      "scenario": "architecture",
      "prompt_template": "Give me a quick overview of the module structure - what are the main entry points?",
      "expected_format": "text_description",
      "scoring": "rubric"
    }
  ]
}
```

### Auto-Selection of Test Parameters

```rust
struct TaskParameterSelector {
    manifest: Manifest,
}

impl TaskParameterSelector {
    fn select_export_symbols(&self, count: usize) -> Vec<String> {
        // Pick exports with varying "findability":
        // 1. One common export (appears in multiple files)
        // 2. One unique export (single file)
        // 3. One nested export (re-exported from multiple levels)

        let exports: Vec<_> = self.manifest.files.iter()
            .flat_map(|f| f.exports.iter().map(|e| (e.clone(), f.file.clone())))
            .collect();

        let mut export_counts: HashMap<String, usize> = HashMap::new();
        for (export, _) in &exports {
            *export_counts.entry(export.clone()).or_default() += 1;
        }

        let unique: Vec<_> = exports.iter().filter(|(e, _)| export_counts[e] == 1).collect();
        let common: Vec<_> = exports.iter().filter(|(e, _)| export_counts[e] > 1).collect();

        let mut selected = vec![];

        if let Some((export, _)) = common.choose(&mut rand::thread_rng()) {
            selected.push(export.clone());
        }

        selected.extend(
            unique.choose_multiple(&mut rand::thread_rng(), count - selected.len())
                .map(|(e, _)| e.clone())
        );

        selected
    }

    fn select_commonly_imported_module(&self) -> String {
        let mut import_counts: HashMap<String, usize> = HashMap::new();

        for file in &self.manifest.files {
            for dep in &file.dependencies {
                *import_counts.entry(dep.clone()).or_default() += 1;
            }
        }

        import_counts.into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(module, _)| module)
            .unwrap_or_default()
    }
}
```

---

## Output Formats

### JSON Output

```json
{
  "job_id": "cmp_abc123",
  "repo": "https://github.com/pmndrs/zustand",
  "branch": "main",
  "commit": "a1b2c3d",
  "completed_at": "2026-01-28T15:30:00Z",
  "repo_stats": {
    "file_count": 16,
    "total_loc": 2850,
    "manifest_size_tokens": 450
  },
  "summary": {
    "control": {
      "total_lines_read": 4250,
      "total_tokens": 38500,
      "total_files_accessed": 28,
      "total_read_calls": 35,
      "accuracy": 0.92
    },
    "fmm": {
      "total_lines_read": 680,
      "total_tokens": 6200,
      "total_files_accessed": 12,
      "total_read_calls": 18,
      "accuracy": 0.96
    },
    "savings": {
      "lines_read": 84.0,
      "tokens": 83.9,
      "files_accessed": 57.1,
      "read_calls": 48.6
    }
  },
  "tasks": [
    {
      "id": "find-export",
      "symbol": "create",
      "control": {
        "lines_read": 890,
        "files_accessed": ["src/index.ts", "src/react.ts", "src/vanilla.ts"],
        "read_calls": 8,
        "tokens": 8100,
        "time_ms": 3200,
        "correct": true
      },
      "fmm": {
        "lines_read": 95,
        "files_accessed": [".fmm/manifest.json", "src/vanilla.ts"],
        "read_calls": 2,
        "tokens": 1150,
        "time_ms": 1400,
        "correct": true
      }
    }
  ]
}
```

### Markdown Report

```markdown
# FMM Comparison Report

**Repository:** [pmndrs/zustand](https://github.com/pmndrs/zustand)
**Branch:** main
**Commit:** a1b2c3d
**Generated:** 2026-01-28 15:30 UTC

---

## Summary

| Metric | Control | FMM | Savings |
|--------|---------|-----|---------|
| Lines Read | 4,250 | 680 | **84.0%** |
| Tokens Used | 38,500 | 6,200 | **83.9%** |
| Files Accessed | 28 | 12 | **57.1%** |
| Read Tool Calls | 35 | 18 | **48.6%** |
| Accuracy | 92% | 96% | +4% |

---

## Conclusion

**FMM reduces token consumption by 84% on this codebase while improving accuracy.**

At scale:
- 100 queries/day: $3.45/day saved
- 1000 queries/day: $34.50/day saved
- Annual projection: $12,593 savings

---

*Generated by fmm compare v1.0.0*
```

---

## Reference: Similar Tools and Services

### Evals Frameworks

| Tool | Focus | Relevance |
|------|-------|-----------|
| **OpenAI Evals** | Model evaluation | Task definition patterns |
| **LangChain Evaluation** | Chain/agent testing | Instrumentation patterns |
| **PromptFoo** | Prompt testing | Comparison methodology |
| **Braintrust** | LLM observability | Metrics collection |

### Benchmarking Suites

| Suite | Focus | Patterns to Borrow |
|-------|-------|-------------------|
| **SWE-bench** | Code generation | Repo setup, sandboxing |
| **HumanEval** | Code completion | Task standardization |
| **MMLU** | Knowledge | Multiple-choice scoring |
| **BigCodeBench** | Code understanding | Multi-repo testing |

### Key Patterns to Adopt

1. **From SWE-bench:** Docker-based sandboxing, git-based version pinning
2. **From PromptFoo:** YAML task definitions, comparison tables
3. **From Braintrust:** Token tracking, cost attribution
4. **From LangChain:** Tool call instrumentation, trace capture

---

## Implementation Roadmap

### Phase 1: Local CLI (Week 1-2)

- [ ] Design document (this file)
- [ ] Core types and traits
- [ ] Clone + manifest generation
- [ ] Single-task test runner
- [ ] Basic instrumentation
- [ ] CLI: fmm compare <url>
- [ ] JSON output

### Phase 2: Full Benchmark Suite (Week 3-4)

- [ ] Task auto-selection
- [ ] Multiple runs with statistics
- [ ] Accuracy scoring
- [ ] Markdown/HTML reports
- [ ] Result caching
- [ ] CLI: --quick, --runs, --output

### Phase 3: REST API (Week 5-6)

- [ ] HTTP server (axum)
- [ ] Job queue (SQLite)
- [ ] Worker pool
- [ ] API endpoints
- [ ] Webhook notifications
- [ ] Basic web UI

### Phase 4: Cloud Ready (Week 7-8)

- [ ] Docker containerization
- [ ] Redis queue option
- [ ] PostgreSQL option
- [ ] Kubernetes manifests
- [ ] Cloud Run deployment
- [ ] Multi-tenant support

---

## Appendix: Configuration Reference

### CLI Configuration (~/.fmm/config.toml)

```toml
[compare]
default_runs = 3
default_task_set = "standard"
cache_dir = "~/.fmm/cache"
max_cache_size_gb = 10

[compare.limits]
max_repo_size_mb = 500
max_files = 5000
max_cost_per_job = 10.0
job_timeout_secs = 1800

[compare.api]
anthropic_model = "claude-opus-4-5-20251101"
temperature = 0.0
max_tokens_per_response = 4096
```

### Environment Variables

```bash
ANTHROPIC_API_KEY=sk-ant-...          # Required
FMM_CACHE_DIR=~/.fmm/cache            # Override cache location
FMM_MAX_COST=10.0                     # Max cost per job
FMM_LOG_LEVEL=info                    # debug|info|warn|error
```

---

*Architecture design completed: 2026-01-28*
*Author: Stuart Robinson with Claude Opus 4.5*
*Status: Ready for implementation review*
