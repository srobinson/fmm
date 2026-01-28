# Authority

**The Human controls this task. The orchestrator speaks for the Human. Follow directives immediately.**

# [ALP-318]: Implement manifest generation (.fmm/index.json)

**Session:** `nancy-ALP-318-iter2`

## What

Generate `.fmm/index.json` manifest for LLM consumption. Update inline frontmatter header to `// --- FMM ---`.

## Why

LLMs skip comments. Manifest JSON is queryable. Self-announcing header enables pattern matching.

## Acceptance Criteria

- [ ] Inline header: `// --- FMM ---`
- [ ] `.fmm/index.json` generated with all file metadata
- [ ] Reverse export index for lookup
- [ ] `update` and `validate` commands support manifest
- [ ] `--manifest-only` flag available

See: `research/exp13/PIVOT.md`

## 0. Working Environment

PROJECT_ROOT: /Users/alphab/Dev/LLM/DEV/fmm

- You are working in a git worktree at: /Users/alphab/Dev/LLM/DEV/fmm-worktrees/nancy-ALP-318
- Branch: nancy/ALP-318
- All your commits stay isolated to this worktree
- Push with: git push -u origin nancy/ALP-318

Use `git log --format=full -10`
 to understand recent changes and context

Use your commit message as a handover to the next developer to facilitation communication and to provide a ledger

## 1. Goal

1. Implement all issues `/Users/alphab/Dev/LLM/DEV/fmm/.nancy/tasks/ALP-318/ISSUES.md` completely.
2. All Success criteria must all pass.

## 2. Work Loop

0. Select issue to work on in list order
1. Use linear-server - get_issue (MCP)(id: ISSUE_ID): Ingest issue
2. Use linear-server - update_issue (MCP)(id: ISSUE_ID): Update state -> "In Progress"
3. Work each task

IMPORTANT
- Use parallel tool calls whenever possible
- Use your tokens wisely :> spawn subagents whenever possible

## 3. Completing an issue

1. Mark complete with an [X] /Users/alphab/Dev/LLM/DEV/fmm/.nancy/tasks/ALP-318/ISSUES.md
2. Use linear-server - update_issue (MCP)(id: ISSUE_ID) to change state to "Worker Done"
3. Commit progress: `nancy[ISSUE_ID]: <detailed_description>`

## 3. Communication

If you have any questions, need more information, get stuck, report status, or to respond to the Orchestrator, use the following protocol:

```bash
nancy msg <msg_type> <message> <priority>

echo "Usage: nancy msg <type> <message> <priority>"
echo ""
echo "Types: blocker, progress, review-request"
echo "Priority: urgent, normal, low (default: normal)"

# For example:
nancy msg progress "Status update here"
nancy msg blocker "Describe what's blocking you"
nancy msg review-request "Ready for review"
```

## 4. Quality

Use `just check` && `just build` && `just test`

Always resolve all issues whether you created it or previous version of yourself ran out of context.

If you catch yourself thinking "There are lint warnings but these are in the code that was already there." you are not being a good citizen. We all work for each other here.

IMPORTANT

Also quality:
Linear is the source of truth. Over any Todos or any other tool.
If Todos is out of sync trust Linear.

## 5. Completion

**Before marking complete, verify:**
1. ALL issues are implemented and tested
2. Inbox is empty (`nancy inbox` shows no pending directives)

**Only then:**
```bash
echo "done" > /Users/alphab/Dev/LLM/DEV/fmm/.nancy/tasks/ALP-318/COMPLETE
```

⚠️ NEVER mark complete with unread messages in your inbox.
