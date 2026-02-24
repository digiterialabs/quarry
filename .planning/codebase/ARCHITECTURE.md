# Architecture

**Analysis Date:** 2026-02-24

## Pattern Overview

**Overall:** Orchestrator-Agent Hub-and-Spoke with Centralized State Management

This is a sophisticated project orchestration system called "Get-Shit-Done" (GSD) that coordinates AI agents through structured workflows. The architecture emphasizes:
- **Parallel agent orchestration** with fresh context isolation per agent
- **Declarative workflows** that guide agent behavior through markdown + frontmatter
- **Atomic state progression** via STATE.md with phase/milestone tracking
- **Tool centralization** in gsd-tools.cjs that all workflows invoke
- **Document-driven execution** where plans become executable prompts for agents

## Layers

**Agent Layer:**
- Purpose: Specialized agents that execute discrete responsibilities (planning, execution, research, verification)
- Location: `.claude/agents/`
- Contains: 11 agent definition files in markdown format that describe role, process, and success criteria
- Depends on: Workflows, tool library, user decisions from CONTEXT.md
- Used by: Orchestrator workflows invoke agents via Task tool

**Workflow Layer:**
- Purpose: Orchestrate multi-step processes, manage user interaction, coordinate agent spawning
- Location: `.claude/get-shit-done/workflows/`
- Contains: 32 workflow files in markdown format describing process steps, checkpoints, and transitions
- Depends on: gsd-tools for state/config operations, agent library for spawning
- Used by: Command handlers invoke workflows as execution context

**Command Layer:**
- Purpose: User-facing entry points that parse arguments and invoke workflows
- Location: `.claude/commands/gsd/`
- Contains: 33 command definition files in markdown specifying allowed tools and context
- Depends on: Workflows to define execution logic
- Used by: Claude Code interprets commands and passes to workflow orchestrators

**Tool Library:**
- Purpose: Centralized utilities for state management, config, phase/milestone operations, verification
- Location: `.claude/get-shit-done/bin/lib/`
- Contains: 11 CommonJS modules (~4,979 lines total):
  - `core.cjs` (398 lines): Model profiles, output helpers, file utilities, config loading
  - `commands.cjs` (556 lines): Slug generation, todos, path verification, history digests
  - `frontmatter.cjs` (299 lines): YAML/frontmatter parsing and extraction
  - `phase.cjs` (873 lines): Phase CRUD, phase listing, phase queries with decimal numbering
  - `state.cjs` (490 lines): STATE.md read/write, field updates, batch patches
  - `roadmap.cjs` (298 lines): ROADMAP.md parsing and phase extraction
  - `milestone.cjs` (215 lines): Milestone completion, archival, version tracking
  - `verify.cjs` (772 lines): Summary verification, consistency checks, health validation
  - `template.cjs` (222 lines): Template scaffolding and pre-filling
  - `init.cjs` (694 lines): Initialization context for workflows
  - `config.cjs` (162 lines): Configuration schema and defaults
- Depends on: File system, git, JSON parsing
- Used by: All workflows via gsd-tools.cjs entry point

**Templates Layer:**
- Purpose: Pre-defined markdown structures for planning documents, milestone archives, state files
- Location: `.claude/get-shit-done/templates/`
- Contains: 15+ template markdown files for project setup, phases, plans, summaries, requirements
- Depends on: Template fill operations from tool library
- Used by: Workflows instantiate templates when creating new phases/plans

**Reference Layer:**
- Purpose: Implementation guides for patterns used throughout workflows (checkpoints, git, TDD, verification)
- Location: `.claude/get-shit-done/references/`
- Contains: 13 markdown reference documents on model profiles, git integration, TDD, verification patterns
- Depends on: None (read-only reference material)
- Used by: Agents and planners reference these when making decisions

**Hooks Layer:**
- Purpose: Monitor session state, check for updates, display status without interrupting Claude
- Location: `.claude/hooks/`
- Contains: 3 Node.js scripts that run during SessionStart and PostToolUse
- Depends on: gsd-tools for state queries
- Used by: Claude Code hooks system runs these transparently

## Data Flow

**Project Lifecycle Flow:**

1. **Initialization (`/gsd:new-project`)**
   - Orchestrator loads init context via `gsd-tools.cjs init new-project`
   - Collects user decisions in CONTEXT.md
   - Creates `.planning/` directory with STATE.md, config.json, ROADMAP.md, REQUIREMENTS.md
   - Spawns project-researcher agent to analyze existing code or requirements
   - Agent produces PROJECT.md with discovery findings
   - Spawns roadmapper agent to break project into phases
   - Roadmapper produces ROADMAP.md with phase descriptions and numbering (1, 1.1, 1.2, 2, etc.)
   - Commits planning docs

2. **Phase Planning (`/gsd:plan-phase`)**
   - Orchestrator loads phase via `gsd-tools.cjs find-phase <number>`
   - Spawns planner agent with codebase docs (ARCHITECTURE.md, CONVENTIONS.md, etc.)
   - Planner decomposes phase into parallel-optimized plans (2-3 tasks per plan)
   - Planner produces PLAN.md files in phase directory with must-haves and dependency graph
   - Optional: Spawns plan-checker agent to verify plan quality
   - Commits PLAN.md files

3. **Execution (`/gsd:execute-phase`)**
   - Orchestrator loads incomplete plans from phase directory via `gsd-tools.cjs phase list-files --type plans`
   - Executor agent reads PLAN.md and implements each task
   - Executor creates per-task git commits (atomic commits)
   - At checkpoints (marked with `[CHECKPOINT]` in plan), executor pauses and requests verification
   - After plan completion, executor produces SUMMARY.md with:
     - Commits executed (SHAs)
     - Files created/modified
     - Self-check verification
     - Deviations from plan (if any)
   - Updates STATE.md with progress
   - Commits SUMMARY.md

4. **Verification (`/gsd:verify-work`)**
   - Orchestrator spawns verifier agent to audit SUMMARY.md
   - Verifier checks commit existence, file presence, artifact quality
   - If gaps found, spawns gap-closure planner via `/gsd:plan-phase --gaps`
   - Gap planner creates additional PLAN-2.md, PLAN-3.md, etc. to close verification gaps
   - Cycle repeats until verification passes

5. **Milestone Completion (`/gsd:complete-milestone`)**
   - Orchestrator marks phases as archived in STATE.md
   - Phases moved from `.planning/phases/` to `.planning/milestones/vX.Y-phases/`
   - MILESTONES.md archive created with completion timestamp and metadata
   - STATE.md advanced to next milestone

**State Progression:**

```
STATE.md (single source of truth)
├── Current Phase/Milestone
├── Plan counter (tracks how many plans executed)
├── Execution metrics (duration, commits, files)
├── Decisions/blockers
└── Progress checkpoints
```

Updates via:
- `gsd-tools.cjs state update <field> <value>` (single field)
- `gsd-tools.cjs state patch --field val1 val2` (batch updates)
- Workflows invoke these after each major step

**Config Resolution:**

```
.planning/config.json
├── model_profile: "balanced" | "quality" | "budget"
├── commit_docs: true/false
├── branching_strategy: "none" | "phase" | "milestone"
├── parallelization: true/false
├── research: true/false
├── plan_checker: true/false
└── verifier: true/false
```

Model profile determines which Claude model runs each agent:
- `quality`: opus (most capable, higher cost)
- `balanced`: sonnet (good quality/cost ratio)
- `budget`: haiku (fastest, lower cost)

See `.claude/get-shit-done/bin/lib/core.cjs` lines 11-23 for MODEL_PROFILES mapping.

## Key Abstractions

**Phase:**
- Purpose: Represents a unit of work with concrete deliverables and timeline
- Location: Defined in `.planning/phases/<number>-<name>/` directories
- Pattern: Each phase contains PLAN.md, SUMMARY.md, possibly multiple plan revisions (PLAN-2.md, etc.)
- Numbering: Decimal system allows insertion (1, 1.1, 1.2, 2, 2.1) without renumbering
- Query: `gsd-tools.cjs find-phase <number>` locates directory and extracts metadata

**Plan:**
- Purpose: Task-level breakdown within a phase, optimized for parallel execution
- Location: `.planning/phases/<number>-<name>/PLAN.md` or `<name>-PLAN.md`
- Structure:
  ```
  ---
  phase: 1
  plan: 1
  wave: 1
  type: execute | tdd
  tasks: 3
  ---

  ## Goal
  ## Dependencies
  ## Tasks
  - Task 1: action [requirements]
  - Task 2: action [requirements]
  ## Must-haves
  - artifacts: [list]
  - key_links: [list]
  ```
- Multiple plans execute in waves (wave 1, 2, 3...) to respect dependencies

**Summary:**
- Purpose: Record of what was executed, used for verification
- Location: `.planning/phases/<number>-<name>/<name>-SUMMARY.md`
- Structure:
  ```
  ---
  phase: 1
  plan: 1
  executed_at: timestamp
  commits: [sha1, sha2, ...]
  duration: "45 min"
  ---

  ## Execution
  - Task 1: [status] - commit abc123
  - Task 2: [status] - commit def456

  ## Files Created
  - src/services/user.ts

  ## Self-Check
  - All must-haves artifacts present
  ```

**Frontmatter (YAML):**
- Purpose: Machine-readable metadata at top of markdown files
- Pattern: Used in PLAN.md, SUMMARY.md, UAT.md, VERIFICATION.md
- Parser: `frontmatter.cjs` extracts and validates
- Validation: `gsd-tools.cjs frontmatter validate <file> --schema plan|summary|verification`

## Entry Points

**Command Entry Point:**
- Location: `.claude/commands/gsd/` directory
- How invoked: User types `/gsd:command-name [args]`
- Responsibility: Parse arguments, load workflow context, invoke orchestrator
- Example: `/gsd:plan-phase 1` → loads `.claude/commands/gsd/plan-phase.md` → invokes plan-phase workflow

**Workflow Entry Point:**
- Location: `.claude/get-shit-done/workflows/<name>.md`
- How invoked: Command handler executes workflow markdown as orchestrator context
- Responsibility: Multi-step process coordination, agent spawning, checkpoint management
- Process: Each `<step>` section in workflow is executed sequentially
- Dependencies: Invokes gsd-tools.cjs for state/config operations

**Agent Entry Point:**
- Location: `.claude/agents/gsd-<type>.md`
- How invoked: Workflow spawns agent via Task tool with fresh context
- Responsibility: Implement specific capability (planning, execution, research, verification)
- Context: Receives project state, codebase docs, user decisions from previous steps
- Output: Creates files directly (PLAN.md, SUMMARY.md, etc.) or returns structured data

## Error Handling

**Strategy:** Progressive recovery with checkpoint pauses

**Patterns:**

1. **Tool Failure:**
   - gsd-tools.cjs commands exit with code 0 (success) or 1 (error)
   - Workflows check output for `@file:` prefix (large JSON payloads written to temp files)
   - On error, workflow pauses and returns error context to user

2. **Agent Deviation:**
   - Plans include "accepted deviations" section
   - If agent deviates, executor logs in SUMMARY.md and pauses at checkpoint
   - User decides whether to continue or re-plan

3. **Verification Failure:**
   - Verifier identifies gaps (missing commits, files, artifacts)
   - Planner creates gap-closure plans
   - New plans execute to fill gaps
   - Verification re-runs until passes

4. **State Corruption:**
   - `gsd-tools.cjs validate health --repair` checks .planning/ integrity
   - Can repair: phase numbering inconsistencies, missing STATE.md, orphaned phases
   - On corruption, pauses and offers repair options

5. **Checkpoint Pause:**
   - Plan contains `[CHECKPOINT: description]` markers
   - Executor pauses at checkpoint, produces partial SUMMARY.md
   - User verifies progress, then resumes execution
   - Prevents runaway execution if unexpected behavior detected

## Cross-Cutting Concerns

**Logging:**
- Approach: Console output via gsd-tools.cjs output() helper
- Formats: JSON (structured) or --raw (plain text for scripts)
- Large payloads (>50KB): Written to temp files, path returned with @file: prefix

**Validation:**
- Frontmatter validation: Phase, plan number, task count match expected structure
- Commit verification: gsd-tools.cjs verify commits <hash1> <hash2>... checks git history
- File verification: Spot-checks 2-3 files mentioned in SUMMARY.md to ensure they exist
- Consistency: Phase numbers must be unique, decimal format, sequential (no gaps)

**Configuration:**
- Model profile selection: Sets which Claude model runs each agent
- Branching strategy: Controls git branch creation (none | phase-based | milestone-based)
- Feature flags: research, plan_checker, verifier, parallelization enable/disable optional workflows
- See `gsd-tools.cjs init <context>` for defaults and overrides

**Git Integration:**
- Commits created by executor are atomic (one commit per task)
- Planning docs committed by workflows using `gsd-tools.cjs commit <message>`
- Branching follows template: `gsd/phase-{phase}-{slug}` or `gsd/{milestone}-{slug}`
- See `.claude/get-shit-done/references/git-integration.md` for full patterns

---

*Architecture analysis: 2026-02-24*
