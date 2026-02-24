# Codebase Concerns

**Analysis Date:** 2026-02-24

## Tech Debt

**Large Agent Files (Maintainability Risk):**
- Issue: Multiple agent definition files exceed 700+ lines each, making modifications error-prone
- Files:
  - `gsd-debugger.md` (1201 lines)
  - `gsd-planner.md` (1194 lines)
  - `gsd-codebase-mapper.md` (764 lines)
  - `gsd-plan-checker.md` (744 lines)
  - `gsd-executor.md` (469 lines)
- Impact: Difficult to navigate, update, or debug agent logic. Changes in one section may inadvertently affect others.
- Fix approach: Consider breaking large agents into focused microagents with clear interfaces, or split each agent into separate sections with table of contents

**Silent Failures in Hook Scripts:**
- Issue: Hooks silently catch and ignore all exceptions without logging
- Files:
  - `.claude/hooks/gsd-context-monitor.js` (lines 118-120: catch all, exit 0)
  - `.claude/hooks/gsd-statusline.js` (lines 105-107: catch all, exit 0)
- Impact: When hooks fail, there's no visibility into what went wrong. Errors in file I/O, JSON parsing, or logic are invisible to users.
- Fix approach: Write errors to a designated log file in `/tmp/` or `.planning/logs/` instead of silently exiting. Include timestamp and error message.

**Fragile File Path Resolution:**
- Issue: gsd-tools relies on relative path patterns to find phase/plan files
- Files: `.claude/get-shit-done/bin/lib/phase.cjs` (lines 59-62 and similar)
- Impact: If file naming convention changes (e.g., `-PLAN.md` → `-plan.md`), phase lookups silently return empty results
- Fix approach: Implement strict validation of phase directory structure with early error reporting. Add migration warnings when legacy naming detected.

**Metadata Bridge Coupling (Inter-Process Communication):**
- Issue: gsd-statusline writes metrics to `/tmp/claude-ctx-{sessionId}.json` and gsd-context-monitor reads it
- Files:
  - `.claude/hooks/gsd-statusline.js` (lines 34-41)
  - `.claude/hooks/gsd-context-monitor.js` (lines 49-55)
- Impact: Race condition possible if writes complete after reads. No locking mechanism. File corruption on power loss. No validation of data format before parsing.
- Fix approach: Add atomic write using temporary file + rename pattern. Add retry logic with exponential backoff on read failures. Validate JSON schema before parsing.

**50KB Output Buffer Limitation:**
- Issue: gsd-tools.cjs detects payloads > 50KB and writes to tmpfile with @file: prefix
- Files: `.claude/get-shit-done/bin/lib/core.cjs` (lines 34-37)
- Impact: Hard-coded 50KB limit may be too conservative or insufficient depending on workflow. No mechanism to adjust threshold.
- Fix approach: Make buffer size configurable via environment variable. Document the threshold and provide guidance on how to increase if needed.

## Known Bugs

**Context Window Scaling Calculation:**
- Symptoms: Context usage percentage displayed to user may not match actual consumption
- Files: `.claude/hooks/gsd-statusline.js` (lines 21-28)
- Trigger: When context usage approaches 80% limit (Claude Code's enforced maximum)
- Cause: Scaling formula `(rawUsed / 80) * 100` distorts the visual representation. At 50% real usage, user sees 62.5%.
- Workaround: Read raw percentage from context_window.remaining_percentage in hook output
- Impact: Users may misjudge when to wrap up work based on displayed percentage

**Debounce Counter Reset Logic:**
- Symptoms: Context warning may fire repeatedly even after debounce is applied
- Files: `.claude/hooks/gsd-context-monitor.js` (lines 79-96)
- Trigger: When severity escalates from WARNING (35%) to CRITICAL (25%)
- Cause: Counter increments before checking debounce (line 79), but severity check bypasses debounce (line 86). On next tool use, counter is reset before next warning, potentially allowing back-to-back warnings.
- Workaround: Disable context monitor if verbose warnings become excessive
- Impact: Repeated context warnings may distract from work

**Empty Catch Blocks Hide Auth Failures:**
- Symptoms: gsd-check-update.js fails silently during npm registry checks
- Files: `.claude/hooks/gsd-check-update.js` (lines 41, 45)
- Trigger: Network timeout or npm registry unreachable
- Cause: Catch blocks assign `latest = null` without logging
- Workaround: Check manually with `npm view get-shit-done-cc version`
- Impact: Users never know if update check failed or if they're on latest version

## Security Considerations

**Unvalidated JSON Parsing from Tmp Files:**
- Risk: gsd-statusline and gsd-context-monitor read and parse JSON from `/tmp/` without schema validation
- Files:
  - `.claude/hooks/gsd-context-monitor.js` (line 49)
  - `.claude/hooks/gsd-statusline.js` (lines 76, 91)
- Exposure: On shared systems, malicious actor could write crafted JSON to tmpfile to trigger unexpected behavior
- Current mitigation: Try-catch blocks prevent crashes, but don't validate data integrity
- Recommendations:
  - Validate JSON schema before parsing (whitelist expected fields)
  - Use cryptographic hash to verify tmpfile hasn't been modified
  - Store tmpfiles with restrictive permissions (0600)
  - Consider using `.planning/` directory instead of `/tmp/` for sensitive data

**Phase Operations Lack Input Sanitization:**
- Risk: Phase names, plan descriptions, and phase numbers not validated
- Files: `.claude/get-shit-done/bin/lib/phase.cjs` (phase add/insert operations)
- Exposure: Could allow injection of special characters into filenames, breaking path assumptions
- Current mitigation: normalizePhaseName function strips some characters
- Recommendations:
  - Use explicit whitelist for allowed characters in phase names (alphanumeric, dash, underscore only)
  - Test injection vectors (spaces, quotes, slashes, newlines)
  - Add unit tests for phase name normalization

**Env Var Handling in Hooks:**
- Risk: Hooks read HOME directory from process.env without validation
- Files: `.claude/hooks/gsd-statusline.js` (line 65), `.claude/hooks/gsd-check-update.js` (line 10)
- Exposure: If HOME is unset or manipulated, path resolution could fail or be redirected
- Current mitigation: Silent failures prevent crashes
- Recommendations:
  - Validate HOME is set and is absolute path before using
  - Add fallback to `os.homedir()` if env var is missing

## Performance Bottlenecks

**Repeated File I/O in Statusline Hook:**
- Problem: gsd-statusline reads from multiple file locations on every tool use
- Files: `.claude/hooks/gsd-statusline.js` (lines 67-84, 88-96)
- Cause: No caching of todo directory or update cache file paths. Reads readdirSync and JSON parses on every status update.
- Impact: If .claude/todos has many files, performance degrades with each tool use
- Improvement path:
  - Cache directory listings for 5-10 second TTL
  - Pre-compile regex filters for file matching
  - Use fs.stat instead of readdirSync when checking single file existence

**Synchronous Phase Lookups:**
- Problem: Phase directory lookups use execSync and sequential file reads
- Files: `.claude/get-shit-done/bin/lib/phase.cjs` (fs.readdirSync, sort, find patterns)
- Cause: No parallelization when searching across multiple phase directories
- Impact: Scales poorly with 50+ phases. Each phase lookup blocks event loop.
- Improvement path:
  - Use async/await with Promise.all() for parallel directory reads
  - Implement phase index cache in `.planning/.phase-index.json` (refreshed on-demand)
  - Consider bloom filter for quick negative lookups

**Large Workflow Files Slow Down Parsing:**
- Problem: Workflows like `new-project.md` (1116 lines) are read entirely into memory
- Files: `.claude/get-shit-done/workflows/new-project.md` and others
- Cause: No lazy loading or section-based parsing
- Impact: Agents load entire workflow even if they only need one section
- Improvement path:
  - Break workflows into smaller focused files
  - Implement `@include` directive for workflow composition
  - Lazy-load only needed sections based on context

## Fragile Areas

**Model Profile Resolution:**
- Files: `.claude/get-shit-done/bin/lib/core.cjs` (lines 11-23)
- Why fragile: Hard-coded agent-to-model mapping. If agent name changes or new agents added, mappings must be manually updated.
- Safe modification:
  - Never rename agents without updating MODEL_PROFILES table
  - Always add new agent mapping before spawning new agent
  - Add unit test to verify all agents in `./agents/` have profiles defined
- Test coverage: No tests exist for model profile resolution fallback

**Phase Numbering Logic:**
- Files: `.claude/get-shit-done/bin/lib/core.cjs` (comparePhaseNum function), `.claude/get-shit-done/bin/lib/phase.cjs`
- Why fragile: Supports multiple phase number formats (integers, decimals, letter suffixes). Edge cases include `1.10` vs `1.2`, `1a` vs `1b`.
- Safe modification:
  - Always test phase comparisons after changes to sorting logic
  - Test with real phase sequences (1, 1.1, 1.1.1, 2, 2.1, 3a)
  - Verify archive/unarchive operations preserve phase ordering
- Test coverage: sortBy logic tested indirectly through phase listing, but no dedicated unit tests

**JSON Frontmatter CRUD Operations:**
- Files: `.claude/get-shit-done/bin/lib/frontmatter.cjs` (lines ~60-100 for set/merge operations)
- Why fragile: YAML/JSON hybrid format. Merge operations assume valid YAML is also valid JSON. No recovery from partial writes.
- Safe modification:
  - Always read full file before writing
  - Use atomic write (temp file + rename) for consistency
  - Validate YAML/JSON syntax after write before returning success
  - Add rollback capability if write fails mid-operation
- Test coverage: No visible tests for frontmatter merge/update operations

**State File Synchronization:**
- Files: `.claude/get-shit-done/bin/lib/state.cjs` (state update/patch operations)
- Why fragile: Multiple workflows may read/write STATE.md simultaneously during parallel phase execution
- Safe modification:
  - Never perform read-modify-write in sequence without locking
  - Always use atomic operations provided by gsd-tools
  - Test concurrent updates to STATE.md with simulated parallel workflows
- Test coverage: No visible concurrency tests

## Scaling Limits

**Phase Directory Structure:**
- Current capacity: ~100 phases before performance degrades
- Limit: File system limits on directory entries, memory usage during readdirSync
- Scaling path:
  - Implement phase archive strategy (move old phases to `.planning/phases/archived/`)
  - Use hash-based directory bucketing (phases/0-9/, phases/10-19/, etc.)
  - Implement lazy loading of phase metadata

**Workflow Agent File Sizes:**
- Current capacity: Agents up to 1200 lines remain readable
- Limit: Beyond 1500 lines, risk of logical errors and difficult debugging
- Scaling path:
  - Break agents into focused microagents (one capability per agent)
  - Use `@include` directives to compose workflows from reusable blocks
  - Implement agent plugin system for extensibility

**Hook Execution Overhead:**
- Current capacity: 3 hooks per session without noticeable slowdown
- Limit: If more than 5-6 hooks added, each tool use incurs cumulative latency
- Scaling path:
  - Consolidate related hooks into single JavaScript file
  - Implement hook batching (run checks less frequently than every tool use)
  - Profile hook execution time and add early exit conditions

## Dependencies at Risk

**npm package: get-shit-done-cc (Update Checking):**
- Risk: Hard dependency on npm registry availability for update checks
- Impact: If npm registry is down, users don't know if updates available, but work continues normally
- Migration plan:
  - Implement fallback to GitHub releases API
  - Add manual update command: `/gsd:check-update --source github`
  - Cache last known version for 7 days before re-checking

**No Locking Mechanism for Concurrent Operations:**
- Risk: Multiple Claude instances could execute phases in parallel, corrupting STATE.md
- Impact: Race conditions in state updates, lost decisions, progress tracking errors
- Migration plan:
  - Implement file-based locking using `.planning/.lock` file with PID
  - Add lock timeout (30 minutes) to prevent stale locks
  - Document serial execution requirement in CLAUDE.md

## Missing Critical Features

**No Rollback/Undo Capability:**
- Problem: Once a phase is executed and committed, there's no built-in way to undo if verification fails
- Blocks: Can't safely retry failed phases without manual git resets
- Solution:
  - Implement phase rollback command: `/gsd:rollback-phase N`
  - Store pre-phase git state in `.planning/checkpoints/`
  - Add confirmation prompt before rollback to prevent accidents

**No Phase Dependency Validation:**
- Problem: Plans declare dependencies in frontmatter, but nothing enforces execution order
- Blocks: Can't detect circular dependencies or missing prerequisite phases
- Solution:
  - Implement dependency graph validation in gsd-tools
  - Add `gsd-tools validate dependencies` command
  - Fail fast if circular dependency detected in phase planning

**No Webhook/Event System for External Integration:**
- Problem: External systems can't subscribe to phase completion, milestone milestones, or work state changes
- Blocks: Can't integrate with Slack, Discord, project management tools
- Solution:
  - Implement event emitter in STATE.md updates
  - Add webhook configuration to `.planning/config.json`
  - Support events: phase-completed, milestone-completed, blocker-added, verification-failed

**No Distributed Context Management:**
- Problem: When context approaches limit, agents have no graceful degradation path
- Blocks: Large workflows get cut off mid-execution with no recovery
- Solution:
  - Implement context quota system in STATE.md
  - Add `gsd-tools context-estimate PLAN.md` to predict context usage
  - Auto-split large plans into smaller chunks if estimated usage > 60%

## Test Coverage Gaps

**Hook Script Error Paths:**
- What's not tested: Error handling in gsd-context-monitor.js and gsd-statusline.js
- Files: `.claude/hooks/gsd-context-monitor.js`, `.claude/hooks/gsd-statusline.js`, `.claude/hooks/gsd-check-update.js`
- Risk: Silent failures mask issues. No visibility into what's happening when hooks fail.
- Priority: HIGH — hooks run on every session and tool use
- Recommendation: Add integration tests that:
  - Corrupt tmpfiles and verify graceful degradation
  - Unset environment variables and verify fallbacks work
  - Simulate network failures in npm registry check

**Phase Numbering Edge Cases:**
- What's not tested: Phase comparison with mixed formats (1, 1.1, 1.1.1, 2a, 2b)
- Files: `.claude/get-shit-done/bin/lib/core.cjs` (comparePhaseNum)
- Risk: Phase ordering could be wrong, breaking phase lookup and progression
- Priority: MEDIUM — affects phase operations but caught by manual testing usually
- Recommendation: Add unit tests with comprehensive phase number sequences

**Concurrent State Updates:**
- What's not tested: Multiple processes updating STATE.md simultaneously
- Files: `.claude/get-shit-done/bin/lib/state.cjs`
- Risk: State corruption, lost updates, inconsistent progress tracking
- Priority: HIGH — could lose work state during parallel phase execution
- Recommendation: Add tests simulating parallel state writers using file locks

**Frontmatter Merge Operations:**
- What's not tested: YAML/JSON roundtrip, partial writes, corrupted input
- Files: `.claude/get-shit-done/bin/lib/frontmatter.cjs`
- Risk: State files could become invalid YAML, breaking all downstream operations
- Priority: MEDIUM — affects PLAN, SUMMARY, VERIFICATION file updates
- Recommendation: Add unit tests for merge with edge cases (nested objects, arrays, special characters)

---

*Concerns audit: 2026-02-24*
