# Codebase Structure

**Analysis Date:** 2026-02-24

## Directory Layout

```
.claude/
├── agents/                           # Agent role definitions
│   ├── gsd-codebase-mapper.md       # Analyzes codebase, writes docs
│   ├── gsd-debugger.md               # Diagnoses and fixes issues
│   ├── gsd-executor.md               # Executes PLAN.md files atomically
│   ├── gsd-integration-checker.md    # Validates external integrations
│   ├── gsd-phase-researcher.md       # Deep-dives into phase context
│   ├── gsd-plan-checker.md           # Reviews plans for quality
│   ├── gsd-planner.md                # Creates executable PLAN.md files
│   ├── gsd-project-researcher.md     # Analyzes existing projects
│   ├── gsd-research-synthesizer.md   # Synthesizes research into docs
│   ├── gsd-roadmapper.md             # Breaks projects into phases
│   └── gsd-verifier.md               # Audits execution and results
├── commands/gsd/                     # User-facing commands (33 total)
│   ├── map-codebase.md               # Analyze codebase structure
│   ├── plan-phase.md                 # Create execution plans
│   ├── execute-phase.md              # Run execution plans
│   ├── verify-work.md                # Audit phase completion
│   ├── new-project.md                # Initialize new project
│   ├── new-milestone.md              # Create milestone checkpoint
│   ├── complete-milestone.md         # Archive milestone
│   ├── add-phase.md                  # Add phase to roadmap
│   ├── research-phase.md             # Research phase context
│   ├── discuss-phase.md              # User decision collection
│   ├── health.md                     # Check system health
│   ├── settings.md                   # Configure workflow behavior
│   └── [29 more command definitions]
├── get-shit-done/
│   ├── VERSION                       # Version string
│   ├── bin/
│   │   ├── gsd-tools.cjs             # Main CLI tool entry point (21,634 lines)
│   │   └── lib/                      # Tool implementations (4,979 lines total)
│   │       ├── core.cjs              # Constants, helpers, model profiles
│   │       ├── config.cjs            # Config schema and defaults
│   │       ├── commands.cjs          # Slug, todos, verification utilities
│   │       ├── frontmatter.cjs       # YAML parsing and validation
│   │       ├── init.cjs              # Initialization context loader
│   │       ├── phase.cjs             # Phase CRUD and queries
│   │       ├── state.cjs             # STATE.md operations
│   │       ├── roadmap.cjs           # ROADMAP.md parsing
│   │       ├── milestone.cjs         # Milestone archival
│   │       ├── template.cjs          # Template scaffolding
│   │       └── verify.cjs            # Verification suite
│   ├── references/                   # Implementation guides (13 docs)
│   │   ├── checkpoints.md            # Checkpoint pause patterns
│   │   ├── git-integration.md        # Git workflow patterns
│   │   ├── tdd.md                    # Test-driven development guide
│   │   ├── verification-patterns.md  # Verification suite patterns
│   │   ├── model-profiles.md         # Model selection guide
│   │   └── [8 more references]
│   ├── templates/                    # Markdown templates (15+ docs)
│   │   ├── project.md                # New project template
│   │   ├── state.md                  # STATE.md structure
│   │   ├── summary.md                # SUMMARY.md structure
│   │   ├── context.md                # CONTEXT.md structure
│   │   ├── plan.md                   # PLAN.md structure
│   │   ├── uat.md                    # UAT.md structure
│   │   ├── requirements.md           # REQUIREMENTS.md structure
│   │   └── [more templates]
│   └── workflows/                    # Orchestration workflows (32 docs)
│       ├── map-codebase.md           # Codebase analysis orchestrator
│       ├── new-project.md            # Project initialization orchestrator
│       ├── plan-phase.md             # Phase planning orchestrator
│       ├── execute-phase.md          # Phase execution orchestrator
│       ├── verify-work.md            # Verification orchestrator
│       ├── complete-milestone.md     # Milestone archival orchestrator
│       └── [26 more workflows]
├── hooks/                            # Git/session hooks
│   ├── gsd-check-update.js           # Check for framework updates
│   ├── gsd-statusline.js             # Display GSD status
│   └── gsd-context-monitor.js        # Monitor context usage
├── settings.json                     # Framework configuration
├── settings.local.json               # User-specific settings
├── gsd-file-manifest.json            # File inventory for debugging
└── package.json                      # CommonJS package marker

.planning/
├── codebase/                         # Codebase analysis docs (written by mapper)
│   ├── STACK.md                      # Technology stack
│   ├── INTEGRATIONS.md               # External integrations
│   ├── ARCHITECTURE.md               # System architecture
│   ├── STRUCTURE.md                  # Directory structure (this file)
│   ├── CONVENTIONS.md                # Coding conventions
│   ├── TESTING.md                    # Testing patterns
│   └── CONCERNS.md                   # Technical debt and issues
├── config.json                       # Project configuration
├── STATE.md                          # Current project state (single source of truth)
├── ROADMAP.md                        # Phase breakdown and timeline
├── REQUIREMENTS.md                   # Project requirements
├── PROJECT.md                        # Project discovery findings
├── CONTEXT.md                        # User decisions and constraints
│
├── phases/                           # Phase execution directory
│   ├── 1-setup/
│   │   ├── 1-setup-PLAN.md          # Plan for phase 1
│   │   ├── 1-setup-SUMMARY.md       # Execution summary
│   │   ├── 1-setup-PLAN-2.md        # Gap closure plan (if needed)
│   │   ├── 1-setup-SUMMARY-2.md     # Gap closure summary
│   │   └── CONTEXT.md               # Phase-specific decisions
│   ├── 1.1-subtask/
│   │   └── [similar structure]
│   └── 2-next-phase/
│       └── [similar structure]
│
├── milestones/                       # Archived milestone phases
│   ├── v0.1-phases/
│   │   └── [archived phase directories]
│   └── MILESTONES.md                # Milestone completion log
│
├── todos/
│   ├── pending/                      # Unresolved todos
│   │   └── [todo-name].md
│   └── completed/                    # Completed todos
│       └── [todo-name].md
│
└── [project root]
    └── CLAUDE.md                    # Project-specific instructions (optional)
```

## Directory Purposes

**`.claude/agents/`:**
- Purpose: Agent role definitions that specify behavior, success criteria, and process
- Contains: 11 markdown files defining distinct agent types
- Key files:
  - `gsd-planner.md`: Creates executable PLAN.md files from phase context
  - `gsd-executor.md`: Implements PLAN.md files atomically with commits
  - `gsd-codebase-mapper.md`: Analyzes codebase, writes ARCHITECTURE.md, CONVENTIONS.md, etc.
  - `gsd-verifier.md`: Audits execution quality against must-haves
- Pattern: Each agent includes role definition, process steps, success criteria, and context requirements

**`.claude/commands/gsd/`:**
- Purpose: User-facing command definitions that specify allowed tools and documentation
- Contains: 33 markdown files, one per command
- Naming: `<command-name>.md` (e.g., `plan-phase.md`, `execute-phase.md`)
- Structure: Frontmatter with description, allowed-tools, then execution context pointing to workflow
- Responsibilities: Parse arguments, validate permissions, load workflow context, invoke orchestrator

**`.claude/get-shit-done/bin/`:**
- Purpose: Executable CLI tool and supporting library
- Key files:
  - `gsd-tools.cjs`: Main entry point (~21,634 lines) - invoked by all workflows
  - Command reference: `node gsd-tools.cjs <command> [args] [--raw]`
  - Subcommands: 100+ operations including state load/update, phase CRUD, verification, templating
- Library:
  - `core.cjs`: Model profiles, output helpers, config loading
  - `phase.cjs`: Phase listing, queries, decimal numbering calculations
  - `state.cjs`: STATE.md field read/write with batch update support
  - `verify.cjs`: Consistency validation, summary verification, health checks

**`.claude/get-shit-done/references/`:**
- Purpose: Implementation guides and patterns for use during planning/execution
- Contains: 13 markdown documents with implementation patterns
- Key references:
  - `checkpoints.md`: How to use [CHECKPOINT] markers in plans
  - `git-integration.md`: Commit message templates and branch naming
  - `verification-patterns.md`: How verifier audits execution artifacts
  - `model-profiles.md`: Guide to claude model selection
- Not templated: Reference material read by agents/planners for decision-making

**`.claude/get-shit-done/templates/`:**
- Purpose: Pre-structured markdown templates for planning documents
- Contains: 15+ reusable templates
- Key templates:
  - `project.md`: PROJECT.md structure for discovery findings
  - `state.md`: STATE.md with all standard fields
  - `plan.md`: PLAN.md with frontmatter, goal, dependencies, tasks, must-haves
  - `summary.md`: SUMMARY.md with execution log and commit tracking
  - `context.md`: CONTEXT.md for user decisions
- Usage: `gsd-tools.cjs template fill <template-type>` instantiates with values

**`.claude/get-shit-done/workflows/`:**
- Purpose: Multi-step orchestration processes for complex operations
- Contains: 32 workflow markdown files
- Key workflows:
  - `map-codebase.md`: Spawn 4 parallel mapper agents
  - `new-project.md`: Initialize project, create roadmap, spawn planner
  - `plan-phase.md`: Create and optionally check plans
  - `execute-phase.md`: Run plans atomically, checkpoint pauses, produce SUMMARY.md
  - `verify-work.md`: Audit execution, detect gaps, spawn gap-closure planner
- Execution: Workflow markdown read sequentially, each `<step>` is an execution instruction

**`.planning/codebase/`:**
- Purpose: Structured codebase analysis documents used by planner/executor
- Contains: 7 markdown documents written by gsd-codebase-mapper
- Documents:
  - `ARCHITECTURE.md`: System patterns, layers, data flow, entry points
  - `STRUCTURE.md`: Directory layout, file locations, naming conventions
  - `STACK.md`: Technology stack (languages, frameworks, dependencies)
  - `INTEGRATIONS.md`: External APIs, databases, auth providers
  - `CONVENTIONS.md`: Naming patterns, code style, import organization
  - `TESTING.md`: Test framework, file locations, patterns
  - `CONCERNS.md`: Technical debt, security issues, fragile areas
- Loaded by: Planner loads relevant docs when creating PLAN.md; Executor references when implementing

**`.planning/phases/`:**
- Purpose: Execution directories for each phase
- Structure: One directory per phase (numbered 1, 1.1, 1.2, 2, 2.1, etc.)
- Contents per phase:
  - `<phase>-PLAN.md`: Executable plan with frontmatter, goal, tasks, must-haves
  - `<phase>-SUMMARY.md`: Execution record with commits, files, verification
  - `<phase>-PLAN-2.md`: Gap-closure plan (if verification identified gaps)
  - `<phase>-SUMMARY-2.md`: Gap-closure execution record
  - `CONTEXT.md`: Phase-specific user decisions and constraints
- Numbering: Decimal allows insertion (1, 1.1, 1.2, 2) without renumbering subsequent phases
- Discovery: `gsd-tools.cjs find-phase <number>` returns directory path

**`.planning/milestones/`:**
- Purpose: Archive completed phase cycles
- Structure: `v<X>.<Y>-phases/` directories with archived phase folders
- Contents: Moved phase directories from `.planning/phases/`
- Metadata: `MILESTONES.md` log with completion timestamps and summaries
- Usage: `gsd-tools.cjs milestone complete <version>` orchestrates archival

**`.planning/`:**
- Purpose: Single source of truth for project state and planning
- Key files:
  - `STATE.md`: Current project state (phase, plan counter, blockers, metrics)
  - `ROADMAP.md`: Phase breakdown with descriptions and sequential numbering
  - `REQUIREMENTS.md`: Project requirements with completion tracking
  - `PROJECT.md`: Project discovery findings (scope, constraints, approach)
  - `CONTEXT.md`: User decisions made during `/gsd:discuss-phase`
  - `config.json`: Workflow configuration (model profile, branching strategy, feature flags)
- Single source of truth: Workflows read from and write to STATE.md after each major operation

## Key File Locations

**Entry Points:**

- Command definitions: `.claude/commands/gsd/<command-name>.md`
  - Example: `.claude/commands/gsd/execute-phase.md`
  - Loads workflow context from `.claude/get-shit-done/workflows/execute-phase.md`

- Workflow entry points: `.claude/get-shit-done/workflows/<name>.md`
  - Orchestrates multi-step process
  - Invokes gsd-tools.cjs and spawns agents

- Agent entry points: `.claude/agents/gsd-<type>.md`
  - Executed in fresh context by workflow Task tool
  - Implements single responsibility (planning, execution, research, verification)

**Configuration:**

- User configuration: `.planning/config.json`
  - Model profile selection
  - Feature flags (research, plan_checker, verifier)
  - Branching strategy
  - Parallelization settings

- Framework configuration: `.claude/settings.json`
  - Session hooks configuration
  - Status line configuration
  - Permission allow-list for tools

- Project instructions: `./CLAUDE.md` (optional)
  - Project-specific guidelines read by executor
  - Security requirements, tech stack constraints, coding conventions

**Core Logic:**

- Tool entry point: `.claude/get-shit-done/bin/gsd-tools.cjs`
  - All workflows invoke `node ./.claude/get-shit-done/bin/gsd-tools.cjs <command> [args]`
  - ~21,634 lines implementing 100+ operations

- Tool library: `.claude/get-shit-done/bin/lib/`
  - `core.cjs`: Model profiles, config loading, helpers
  - `phase.cjs`: Phase CRUD, decimal numbering, queries
  - `state.cjs`: STATE.md read/write operations
  - `verify.cjs`: Consistency and health validation
  - `frontmatter.cjs`: YAML metadata parsing

**Testing & Validation:**

- Health checks: `gsd-tools.cjs validate health [--repair]`
  - Checks phase numbering, STATE.md existence, orphaned phases
  - Repairs common issues

- Consistency validation: `gsd-tools.cjs validate consistency`
  - Verifies phase disk state matches ROADMAP.md
  - Checks for missing plans/summaries

- Verification suite: `gsd-tools.cjs verify plan-structure <file>`
  - Validates PLAN.md frontmatter
  - Checks task structure, must-haves format

## Naming Conventions

**Files:**

- Planning documents: `STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md` (UPPERCASE)
- Phase plans: `<phase>-PLAN.md`, `<phase>-PLAN-2.md` (descriptive + PLAN suffix)
- Execution summaries: `<phase>-SUMMARY.md`, `<phase>-SUMMARY-2.md` (descriptive + SUMMARY suffix)
- Commands: `<command-name>.md` (kebab-case)
- Workflows: `<workflow-name>.md` (kebab-case)
- Agents: `gsd-<agent-type>.md` (gsd- prefix, kebab-case)
- CLI tools: `gsd-tools.cjs` (main entry), `*.cjs` (CommonJS modules)

**Directories:**

- Framework structure: `.claude/` (hidden, framework configuration)
  - Subdirectories: `agents/`, `commands/`, `get-shit-done/`, `hooks/`, `worktrees/`
- Planning structure: `.planning/` (hidden, project planning)
  - Subdirectories: `codebase/`, `phases/`, `milestones/`, `todos/`, `config.json`, `STATE.md`
- Phase directories: `.planning/phases/<number>-<slug>/`
  - Format: `1`, `1.1`, `1.2`, `2`, `2.1` (decimal, supports insertion)
  - Slug: kebab-case derived from phase name

**Phase Numbering:**

- System: Decimal numbering allows insertion without renumbering
- Examples: 1, 1.1, 1.2, 2, 2.1, 2.1.1
- Calculation: `gsd-tools.cjs phase next-decimal <after-phase>` computes next number
- Insertion: `gsd-tools.cjs phase insert <after> "<description>"` creates decimal phase

## Where to Add New Code

**New Workflow Command:**

1. Create command definition: `.claude/commands/gsd/<command-name>.md`
   - Specify allowed tools
   - Load workflow context

2. Create workflow: `.claude/get-shit-done/workflows/<command-name>.md`
   - Define `<step>` sections
   - Use gsd-tools.cjs for state/config operations
   - Spawn agents as needed

**New Agent Type:**

1. Create agent: `.claude/agents/gsd-<type>.md`
   - Define role and responsibilities
   - Describe process steps with explicit instructions
   - Document context requirements and success criteria

2. Reference in workflows: Workflows spawn via Task tool with agent name

**New Tool Operation:**

1. Add to gsd-tools.cjs main entry: Parse command and invoke handler
2. Implement in appropriate `.claude/get-shit-done/bin/lib/` module
3. Export function in module, require in gsd-tools.cjs
4. Document in gsd-tools.cjs header with usage examples

**New Template:**

1. Create template: `.claude/get-shit-done/templates/<name>.md`
2. Use placeholder syntax: `[FIELD_NAME]` for substitution
3. Implement fill operation in `template.cjs`
4. Invoke via `gsd-tools.cjs template fill <name> --field val`

**Utilities:**

- Shared helpers: Add to `.claude/get-shit-done/bin/lib/core.cjs`
- Specific domains: Add to appropriate module (phase.cjs, state.cjs, verify.cjs)
- Reference material: Add to `.claude/get-shit-done/references/`

## Special Directories

**`.claude/`:**
- Purpose: Framework configuration and executable code
- Generated: No (hand-written)
- Committed: Yes
- Permissions: Read by all workflows/agents, written by developer updates only

**`.planning/codebase/`:**
- Purpose: Codebase analysis documents
- Generated: Yes (by gsd-codebase-mapper agents)
- Committed: Yes
- Usage: Planner loads when creating PLAN.md; Executor references when implementing

**`.planning/phases/`:**
- Purpose: Phase execution workspace
- Generated: Yes (by workflows creating phase directories)
- Committed: Yes (plans, summaries, and phase context)
- Pattern: New phase directory created by `gsd-tools.cjs phase add`

**`.planning/milestones/`:**
- Purpose: Archive completed phase cycles
- Generated: Yes (by complete-milestone workflow)
- Committed: Yes (archived phase directories)
- Pattern: Phases moved here by `gsd-tools.cjs milestone complete <version>`

**`.planning/todos/`:**
- Purpose: Track pending and completed tasks
- Generated: Yes (by add-todo command)
- Committed: Depends on project policy
- Pattern: Each todo is a separate markdown file

**`.claude/worktrees/`:**
- Purpose: Track git worktree branches (if branching strategy enabled)
- Generated: Yes (if branching_strategy != "none")
- Committed: Yes
- Pattern: Maps phase/milestone numbers to git branch names

---

*Structure analysis: 2026-02-24*
