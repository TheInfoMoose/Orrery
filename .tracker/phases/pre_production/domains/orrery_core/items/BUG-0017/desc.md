## Problem Description
`ProjectProfile::from_project` (`crates/orrery-core/src/config.rs`, §9.19.8) records skills for every role with non-empty `skills`, including custom roles (project or global). `init_project_with_profile` (§9.19.9) rejects any `role_skills` key that is not a default role (`orchestrator`, `coder`, `validator`) with "profile sets skills for unknown default role '<id>'". So `orrery profile save X` in a project that has e.g. a `reviewer` role with skills produces a profile that `orrery init --profile X` always refuses. Suggested fix: have `from_project` keep only default role ids (or warn on save about dropped custom roles), so save and init agree.
Found during: STEP 109 hand-off review.

## Reproduction Steps
1. `orrery init` in a scratch dir; `orrery role add reviewer ...`; `orrery role skills reviewer house-style`.
2. `orrery profile save p` → succeeds.
3. In another dir: `orrery init --profile p` → Validation error "profile sets skills for unknown default role 'reviewer'".

## Fix Log & Agent Notes
