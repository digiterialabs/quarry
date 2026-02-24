# Coding Conventions

**Analysis Date:** 2026-02-24

## Naming Patterns

**Files:**
- kebab-case for hook files: `gsd-check-update.js`, `gsd-statusline.js`, `gsd-context-monitor.js`
- Prefix with feature/module name: `gsd-*` for "Get Shit Done" system hooks
- Extension: `.js` for Node.js scripts

**Functions:**
- Arrow functions preferred: `process.stdin.on('data', chunk => input += chunk)`
- Named constants for reusable values: `const WARNING_THRESHOLD = 35`
- Callback-style for async operations: `spawn()` with completion handled via `unref()`

**Variables:**
- camelCase for all variables: `homeDir`, `cacheFile`, `projectVersionFile`, `remaining`, `usedPct`
- ALL_CAPS for configuration constants: `WARNING_THRESHOLD`, `CRITICAL_THRESHOLD`, `STALE_SECONDS`, `DEBOUNCE_CALLS`
- Single-letter for temporary loop variables: `e` for caught exceptions
- Descriptive names for domain logic: `inProgress`, `severityEscalated`, `firstWarn`

**Types:**
- JSON objects with snake_case keys in serialization: `update_available`, `remaining_percentage`, `used_pct`, `checked`, `session_id`
- Consistent key naming across modules for interoperability between hooks

## Code Style

**Formatting:**
- No explicit formatter detected - manual formatting observed
- Indentation: 2 spaces (Node.js convention)
- Line length: Varies, but generally under 100 characters in practice
- No trailing semicolons enforced (not used on module level, but present in function bodies)

**Linting:**
- No ESLint configuration detected
- No Prettier configuration detected
- Code appears written with manual adherence to conventions

## Import Organization

**Order:**
1. Built-in Node.js modules: `fs`, `path`, `os`, `child_process`
2. Constants definition
3. Module initialization code

**Pattern from `/Users/krishnakumar/Code/quarry/.claude/hooks/gsd-statusline.js`:**
```javascript
const fs = require('fs');
const path = require('path');
const os = require('os');

// Then used immediately in module initialization
let input = '';
process.stdin.setEncoding('utf8');
```

**Path Aliases:**
- Relative paths used exclusively: `path.join()` for cross-platform compatibility
- Environment-relative paths: `os.homedir()` for home directory, `process.cwd()` for current working directory, `os.tmpdir()` for temporary files

## Error Handling

**Patterns:**
- Silent failures with empty catch blocks: `} catch (e) {}` used when failure is non-critical
- Rationale documented in comments: "Silent fail -- don't break statusline on parse errors"
- Graceful degradation: Functions continue execution with default values when exceptions occur
- Exit codes: `process.exit(0)` used to signal clean termination from hook scripts

**Examples from codebase:**
```javascript
// Pattern 1: Silent failure with fallback
try {
  latest = execSync('npm view get-shit-done-cc version', { encoding: 'utf8', timeout: 10000 }).trim();
} catch (e) {}

// Pattern 2: Silent failure on non-critical operations
try {
  fs.writeFileSync(bridgePath, bridgeData);
} catch (e) {
  // Silent fail -- bridge is best-effort, don't break statusline
}

// Pattern 3: Debounced warnings instead of immediate errors
if (!fs.existsSync(metricsPath)) {
  process.exit(0);  // Exit silently if expected file doesn't exist
}
```

## Logging

**Framework:** `console` not used - output via `process.stdout.write()` and `process.exit()`

**Patterns:**
- Structured JSON output for tool integration: `JSON.stringify(output)` for PostToolUse hook
- Status messages as plain text via stdout: `process.stdout.write()` for statusline display
- No console.log() detected - scripts designed for integration with Claude hooks
- Comments document behavior: `// BIG SCARY WARNING` style comments in concatenated strings

**Example from `/Users/krishnakumar/Code/quarry/.claude/hooks/gsd-context-monitor.js` line 101:**
```javascript
message = `CONTEXT MONITOR CRITICAL: Usage at ${usedPct}%. Remaining: ${remaining}%. ` +
  'STOP new work immediately. Save state NOW and inform the user that context is nearly exhausted. ' +
  'If using GSD, run /gsd:pause-work to save execution state.';
```

## Comments

**When to Comment:**
- Complex logic: Context window scaling calculations documented with inline comments
- Workflow documentation: Multi-step processes documented with numbered comments
- Intent explanation: Comments explain "why" rather than "what"

**Style:**
```javascript
// Short comment on single line
// Multiple lines for complex explanations
// Line 1: Context
// Line 2: More context
```

**Examples from code:**
```javascript
// Line 21-22 in gsd-statusline.js: Explains context window calculation
// Context window display (shows USED percentage scaled to 80% limit)
// Claude Code enforces an 80% context limit, so we scale to show 100% at that point

// Line 30-31: Explains cross-file communication
// Write context metrics to bridge file for the context-monitor PostToolUse hook.
// The monitor reads this file to inject agent-facing warnings when context is low.
```

## Function Design

**Size:** Functions are typically medium-sized (20-60 lines) with clear single purposes
- Update check: ~15 lines of actual logic
- Statusline generation: ~95 lines including I/O and branching
- Context monitoring: ~90 lines with state management and warnings

**Parameters:**
- Minimal parameters - functions designed for Node.js callback patterns
- Data passed via stdin/files rather than function arguments
- Callback functions: `.on('data')`, `.on('end')` patterns used for streaming input

**Return Values:**
- Functions don't return values - they write to stdout or files
- Hook scripts signal success via `process.exit(0)`
- Silent failures on non-critical operations (catch blocks with empty body)

**Pattern example from `/Users/krishnakumar/Code/quarry/.claude/hooks/gsd-check-update.js`:**
```javascript
// No return - writes to file and exits
const child = spawn(process.execPath, ['-e', `...`], {
  stdio: 'ignore',
  windowsHide: true,
  detached: true
});
child.unref();  // Function ends here, background process continues
```

## Module Design

**Exports:**
- No exports detected - files are standalone scripts
- Each hook runs as independent process via `node .claude/hooks/gsd-*.js`
- Communication via files, stdout, and environment

**Integration Pattern:**
- stdin → Processing → stdout/files
- Scripts configured in `/Users/krishnakumar/Code/quarry/.claude/settings.json` hooks section
- Minimal coupling: Each script responsible for one hook event (SessionStart, PostToolUse, statusLine)

**Files:**
- `gsd-check-update.js`: Version check logic - spawns background process
- `gsd-statusline.js`: Status display - reads todos and metrics from files
- `gsd-context-monitor.js`: Context tracking - manages warning state and thresholds

## Concurrency & Async Patterns

**Callbacks:**
- stdin streaming: `process.stdin.on('data')` and `process.stdin.on('end')`
- Graceful termination: `process.exit()` after processing
- No promises or async/await detected

**Background Processes:**
- `spawn()` with `detached: true` for background execution
- `child.unref()` to allow parent process to exit
- `stdio: 'ignore'` to detach output streams

**File Operations:**
- Synchronous operations: `fs.readFileSync()`, `fs.writeFileSync()`, `fs.existsSync()`
- Rationale: Hook scripts are short-lived, synchronous I/O is acceptable

## Environment & Configuration

**Environment Variables:**
- Not explicitly used in code
- System paths via `os.homedir()`, `os.tmpdir()`, `process.cwd()`

**Configuration:**
- Thresholds defined as module constants at top of files
- Hardcoded paths use standard directories (home, temp, cwd)
- Spawned processes pass configuration via string substitution: `${JSON.stringify(variable)}`

---

*Convention analysis: 2026-02-24*
