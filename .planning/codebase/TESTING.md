# Testing Patterns

**Analysis Date:** 2026-02-24

## Test Framework

**Runner:**
- Not detected - No test framework configured
- No `jest.config.js`, `vitest.config.js`, or similar test configuration files found
- No `.test.js` or `.spec.js` files in repository

**Assertion Library:**
- Not applicable - No automated tests present

**Manual Testing Approach:**
- Code designed for integration testing via Claude hooks
- Verification via file output inspection and process behavior observation
- No unit test infrastructure

## Test File Organization

**Location:**
- Not applicable - No test files present

**Strategy:**
- Manual testing via hook integration with Claude Code
- Hooks can be tested by running them standalone: `node .claude/hooks/gsd-check-update.js`
- Integration testing via Claude Code session that triggers hooks

## Test Structure

**Manual Testing Approach:**

The codebase uses integration-level testing through Claude's hook system. Each hook can be tested independently by:

1. **Background Process Verification** (`gsd-check-update.js`):
   - Manual: Run `node .claude/hooks/gsd-check-update.js` and verify cache file created
   - Expected output: `/home/.claude/cache/gsd-update-check.json` with fields: `update_available`, `installed`, `latest`, `checked`

2. **Statusline Testing** (`gsd-statusline.js`):
   - Manual: Pipe JSON input via stdin
   - Example: `echo '{"model":{"display_name":"Claude"},"workspace":{"current_dir":"/tmp"},"session_id":"test"}' | node .claude/hooks/gsd-statusline.js`
   - Verify: ANSI-formatted output suitable for terminal statusline

3. **Context Monitor Testing** (`gsd-context-monitor.js`):
   - Manual: Create bridge metrics file and pipe JSON input
   - Expected behavior: Generates warning messages when remaining context <= 35%
   - Verify: Warning debouncing (every 5 tool uses)
   - Verify: Severity escalation (CRITICAL immediately overrides debounce)

## Mocking

**Framework:** Not applicable - No test framework

**File System Mocking:**
Codebase relies on actual file system interactions for testing. Key files to prepare for manual testing:

- `/tmp/claude-ctx-{session_id}.json` - Context metrics bridge file
- `~/.claude/cache/gsd-update-check.json` - Version check cache
- `~/.claude/todos/{session_id}-agent-*.json` - Current task state

**Process Mocking:**
- Background process spawning in `gsd-check-update.js` uses `spawn()` with `detached: true`
- Cannot easily mock external `npm` command - depends on npm registry accessibility
- Timeout handling: `execSync()` with 10-second timeout

## Fixtures and Factories

**Test Data Patterns:**

From `/Users/krishnakumar/Code/quarry/.claude/hooks/gsd-statusline.js`:
```javascript
// Fixture-like test input for statusline
const testInput = {
  model: { display_name: 'Claude' },
  workspace: { current_dir: '/Users/example/project' },
  session_id: 'abc-123',
  context_window: { remaining_percentage: 50 }
};

// Pipe via: echo '...' | node gsd-statusline.js
```

From `/Users/krishnakumar/Code/quarry/.claude/hooks/gsd-context-monitor.js`:
```javascript
// Fixture: Context metrics file
{
  session_id: 'abc-123',
  remaining_percentage: 30,  // Below WARNING_THRESHOLD of 35
  used_pct: 70,
  timestamp: Math.floor(Date.now() / 1000)
}

// Fixture: Warning debounce state
{
  callsSinceWarn: 2,
  lastLevel: 'warning'
}
```

**Location:**
- Temporary directory: `/tmp/claude-ctx-*.json`
- Home directory: `~/.claude/cache/`, `~/.claude/todos/`
- Test data is created/destroyed during manual testing

## Coverage

**Requirements:** None enforced - No test framework

**Current State:**
- Manual coverage: All three hooks have observable I/O points
- Critical paths: Version checking, statusline rendering, context warning system
- Uncovered areas: Edge cases in file parsing, process spawning failures, concurrent file access

**Testing Gaps:**
- No unit test coverage
- No automated regression tests
- No CI/CD test pipeline
- No coverage metrics

## Test Types

**Integration Tests (Manual):**
- Hook runs within Claude Code session
- Validates file creation/reading
- Checks stdout formatting for statusline
- Verifies JSON output structure for PostToolUse hooks

**End-to-End Testing:**
- Full Claude Code session with hooks enabled
- Verify statusline displays correctly
- Trigger high-context scenarios to test warnings
- Check update notifications

**Unit Tests:**
- Not implemented - Would benefit from testing pure functions:
  - Context scaling logic (gsd-statusline.js lines 26-28)
  - Warning threshold logic (gsd-context-monitor.js lines 81-91)
  - File path resolution (all three files)

## Common Patterns

**Async Input Processing Pattern:**
```javascript
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => input += chunk);
process.stdin.on('end', () => {
  try {
    const data = JSON.parse(input);
    // Process data
  } catch (e) {
    // Silent fail
  }
});
```

**Safe File Reading Pattern:**
```javascript
try {
  if (fs.existsSync(filePath)) {
    const content = fs.readFileSync(filePath, 'utf8');
    const data = JSON.parse(content);
    // Use data
  }
} catch (e) {
  // Silent fail - file missing or corrupted
}
```

**Debounce State Management:**
```javascript
const statePath = path.join(tmpDir, `app-state-${sessionId}.json`);
let state = { callCount: 0, lastAction: null };

if (fs.existsSync(statePath)) {
  try {
    state = JSON.parse(fs.readFileSync(statePath, 'utf8'));
  } catch (e) {
    // Reset to default on corruption
  }
}

// Update and persist
state.callCount++;
fs.writeFileSync(statePath, JSON.stringify(state));
```

**Process Spawning Pattern (Background Task):**
```javascript
const child = spawn(process.execPath, ['-e', `
  // Code to run in background
  const result = doSomething();
  fs.writeFileSync(resultPath, JSON.stringify(result));
`], {
  stdio: 'ignore',
  windowsHide: true,
  detached: true
});

child.unref();  // Allow parent to exit
```

## Manual Testing Checklist

For contributors without automated test framework:

**gsd-check-update.js:**
- [ ] Run hook manually: `node .claude/hooks/gsd-check-update.js`
- [ ] Verify cache file created: `cat ~/.claude/cache/gsd-update-check.json`
- [ ] Check JSON structure: `installed`, `latest`, `update_available`, `checked` fields present
- [ ] Verify background process exits cleanly (no output to stdout)

**gsd-statusline.js:**
- [ ] Create test input JSON with all required fields
- [ ] Pipe input: `echo '{"model":...}' | node .claude/hooks/gsd-statusline.js`
- [ ] Verify ANSI color codes in output (check for `\x1b[` escape sequences)
- [ ] Test with/without context_window data
- [ ] Test with/without session_id (should degrade gracefully)

**gsd-context-monitor.js:**
- [ ] Create session ID: `SESSION=test-$(date +%s)`
- [ ] Create metrics bridge file: `echo '{"remaining_percentage":30,...}' > /tmp/claude-ctx-$SESSION.json`
- [ ] Create input JSON with session_id
- [ ] Verify warning output when remaining <= 35%
- [ ] Verify no output when remaining > 35%
- [ ] Test debounce: 5 tool uses before next warning
- [ ] Test severity escalation: WARNING -> CRITICAL should skip debounce

---

*Testing analysis: 2026-02-24*
