# Workspace Refactor & Public Presentation Spec

**Status:** Draft  
**Date:** 2026-10-18  
**Scope:** All 23 repos across active development workspace  

## Context

Our tools are getting good enough to showcase publicly. Before someone with worse practices gets ahold of similar patterns, we need to:
1. Establish consistent best practices across all repos
2. Prepare for public presentation (clean READMEs, docs, licenses)
3. Ensure code quality is production-grade and maintainable

## Current State

- **23 repositories** total (mix of public/private)
- Primary projects: crabjar, mirror-lab, pesti, llm-workspace
- Supporting tools: herdr, various MCP servers, deployment scripts
- Inconsistent standards across repos (some have CI, some don't)
- Private repos contain sensitive patterns worth protecting

## Refactor Stages

### Stage 1: Audit & Inventory (Week 1)

**Goal:** Complete understanding of current state across all repos.

Tasks:
- [ ] Catalog all 23 repos with purpose, language, dependencies
- [ ] Identify public vs private and why
- [ ] Assess code quality per repo (tests, docs, CI)
- [ ] Identify shared patterns that could be standardized
- [ ] Document current secrets/credentials exposure risk
- [ ] Map inter-repo dependencies

**Deliverable:** `docs/workspace-audit.md` with inventory table and risk assessment.

### Stage 2: Standardization Framework (Week 1-2)

**Goal:** Define best practices to apply across all repos.

Tasks:
- [ ] Create `CONTRIBUTING.md` template for all repos
- [ ] Standardize README structure (purpose, quickstart, architecture, usage)
- [ ] Define CI pipeline standards (lint, test, build, deploy stages)
- [ ] Establish testing coverage minimums per repo type
- [ ] Create license selection guide and apply consistently
- [ ] Define versioning strategy (semver vs date-based)
- [ ] Document security best practices for public repos

**Deliverable:** `docs/standards.md` with all templates and guidelines.

### Stage 3: Core Repos Refactor (Weeks 2-4)

**Goal:** Bring primary projects to public-ready quality.

Priority order: crabjar → mirror-lab → pesti → llm-workspace

For each repo:
- [ ] Update README with comprehensive documentation
- [ ] Add/fix CI pipeline matching standards
- [ ] Ensure test coverage meets minimums
- [ ] Verify all dependencies are up-to-date and pinned
- [ ] Clean up TODO/FIXME comments or document as issues
- [ ] Add CHANGELOG.md if missing
- [ ] Verify license file is correct and complete
- [ ] Run security audit (cargo audit, npm audit, etc.)

**Deliverable:** Public-ready core repos with consistent quality.

### Stage 4: Supporting Tools Refactor (Weeks 4-6)

**Goal:** Bring all remaining repos to acceptable quality.

Tasks:
- [ ] Apply standardization framework to all supporting tools
- [ ] Consolidate duplicate utilities into shared packages where appropriate
- [ ] Update deployment scripts to use standardized patterns
- [ ] Document internal tooling for public consumption
- [ ] Verify cross-repo integration points work correctly

**Deliverable:** All 23 repos at consistent quality level.

### Stage 5: Public Presentation Prep (Weeks 6-7)

**Goal:** Prepare for public showcase and community engagement.

Tasks:
- [ ] Create unified documentation site or docs repo
- [ ] Write blog posts / technical writeups showcasing key innovations
- [ ] Prepare demo scripts and example projects
- [ ] Set up issue templates and contribution workflows
- [ ] Create social media presence (GitHub org profile, etc.)
- [ ] Plan announcement strategy (staged reveals vs big bang)

**Deliverable:** Public-facing documentation and marketing materials.

### Stage 6: Ongoing Maintenance (Continuous)

**Goal:** Maintain quality standards as development continues.

Tasks:
- [ ] Implement automated quality checks in CI
- [ ] Schedule regular dependency updates
- [ ] Monitor for security vulnerabilities
- [ ] Review and update documentation regularly
- [ ] Engage with community feedback

## Best Practices to Enforce

### Code Quality
- All public repos must pass `cargo clippy -- -D warnings` (Rust) or equivalent
- Test coverage minimum: 70% for core logic, 50% for integration code
- No unwrap() in production code (use ? or explicit error handling)
- All external dependencies pinned to specific versions

### Documentation
- Every repo must have: README.md, CONTRIBUTING.md, LICENSE
- Public APIs must have doc comments with examples
- Architecture decisions documented as ADRs
- CHANGELOG maintained for all user-facing changes

### CI/CD
- All repos must have automated testing on push
- Build reproducibility enforced (deterministic builds)
- Security scanning integrated into pipeline
- Release automation for tagged versions

### Security
- No secrets in public repos (use .env.example with placeholders)
- Dependency vulnerability scanning required
- Regular rotation of credentials and tokens
- Access control documented for private repos

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking changes during refactor | Use feature branches, test thoroughly before merge |
| Loss of functionality | Maintain comprehensive test suite throughout |
| Inconsistent standards adoption | Create tooling/scripts to automate checks |
| Public exposure of sensitive patterns | Audit private repos for anything worth keeping private |
| Scope creep | Strict adherence to staged approach with clear deliverables |

## Success Criteria

1. All 23 repos pass standardized quality checks
2. Core 4 repos (crabjar, mirror-lab, pesti, llm-workspace) are public-ready
3. Consistent documentation and contribution workflow across all repos
4. Automated CI/CD pipelines in place for all repos
5. Security audit completed with no critical vulnerabilities
6. Public presentation materials ready for showcase

## Prioritized Refactor Checklist (Local-First Approach)

### Phase 1: Quick Wins (Day 1-2)
1. [x] Add LICENSE files to home-lab, Agendarator, user_coarses
2. [x] Create README.md for mirror-lab, Agendarator, user_coarses
3. [x] Remove Google client secret from Agendarator repo (added to .gitignore)
4. [x] Clean up duplicate pesti checkout (backed up old one)

### Phase 2: CI/CD Infrastructure (Week 1)
5. [x] Add GitHub Actions CI to mirror-lab (lint, test, build)
6. [ ] Add GitHub Actions CI to pesti (already has comprehensive CI — verified)
7. [ ] Standardize CI templates across all Rust repos

### Phase 3: Documentation Overhaul (Week 1-2)
8. [x] Write comprehensive README for mirror-lab (architecture, usage, examples)
9. [ ] Update home-lab README with deployment instructions
10. [ ] Structure user_coarses as public educational content
11. [x] Add CHANGELOG.md to all active repos (mirror-lab done, others pending)

### Phase 4: Code Quality & Testing (Week 2-3)
12. [ ] Ensure all Rust repos pass `cargo clippy -- -D warnings`
13. [ ] Add missing unit tests for critical paths
14. [ ] Establish test coverage standards and measure current state
15. [ ] Fix any TODO/FIXME comments or document as issues

### Phase 5: Security Hardening (Week 3)
16. [ ] Audit all repos for exposed secrets/credentials
17. [ ] Run cargo audit on all Rust projects
18. [ ] Pin all dependencies to specific versions
19. [ ] Document security considerations in each repo

### Phase 6: Public Presentation Prep (Week 4)
20. [ ] Create unified documentation site or docs repo
21. [ ] Write technical blog posts showcasing key innovations
22. [ ] Prepare demo scripts and example projects
23. [ ] Set up GitHub org profile and social media presence

## Local-First Deployment Strategy

1. **Local development**: Make all changes in ~/projects/active/ repos ✅ (in progress)
2. **Local testing**: Verify everything works locally before pushing
3. **Remote push**: Push to GitHub upstream when satisfied
4. **Downstream deployment**: Update homelab machines via home-lab manifests

This ensures we can iterate quickly locally without affecting the fleet until ready.

## Status as of 2026-10-18

### Completed
- Phase 1: All quick wins done (LICENSE, READMEs, gitignore cleanup, duplicate pesti checkout)
- Phase 2: CI added to mirror-lab; pesti already had comprehensive CI
- Phase 3: Mirror-lab README and CHANGELOG complete
- Local changes committed across all active repos
- Remote push completed for: mirror-lab, crabjar, pesti, home-lab

### In Progress
- Security vulnerability remediation (GitHub reported issues in crabjar and home-lab)
- Phase 3 continuation (home-lab README, user_coarses structure)

### Notes
- Agendarator and user_coarses are local-only repos (no remote configured)
- GitHub Dependabot found vulnerabilities:
  - crabjar: 5 (3 moderate, 2 low) — https://github.com/CromboJambo/crabjar/security/dependabot
  - home-lab: 8 (5 high, 3 moderate) — https://github.com/CromboJambo/home-lab/security/dependabot

### Next Steps
1. Address security vulnerabilities in crabjar and home-lab
2. Continue Phase 3 (home-lab README, user_coarses structure)
3. Begin Phase 4 (code quality & testing improvements)
