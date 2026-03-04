# Quarry v0.1.0 Release Checklist

## Branch and scope

- [ ] Work from `codex/release-v0.1.0`
- [ ] Merge latest `origin/main` into release branch
- [ ] Confirm non-product artifacts are excluded from release branch (`.planning`, `.claude`, `.superset`, `idea.md`)

## Build and test

- [ ] `cargo fmt --check`
- [ ] `cargo test -q`
- [ ] `python3 tests/mcp_smoke.py`
- [ ] Installer smoke tests:
  - [ ] `python3 scripts/install_integrations.py --codex`
  - [ ] `python3 scripts/install_integrations.py --claude`
  - [ ] `python3 scripts/install_integrations.py --cursor`

## Functional checks

- [ ] `quarry validate --model models/example/model.yml`
- [ ] `quarry query ... --tenant tenant_123` returns tenant_123 aggregates
- [ ] `quarry query ... --tenant tenant_999` returns distinct tenant_999 aggregates
- [ ] `quarry explain ...` returns plan payload

## Documentation

- [ ] README quickstart is accurate
- [ ] Integration docs for Codex / Claude Code / Cursor are linked
- [ ] License file present and correct

## GitHub release

- [ ] CI green on release PR
- [ ] Merge PR into `main`
- [ ] Tag `v0.1.0` on merged `main`
- [ ] Verify GitHub Release assets for Linux/macOS/Windows
- [ ] Verify `SHA256SUMS`
