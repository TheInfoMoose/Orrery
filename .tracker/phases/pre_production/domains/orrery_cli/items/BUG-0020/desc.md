## Problem Description
`orrery profile save` (`crates/orrery-cli/src/commands.rs` `dispatch_profile`, via `ProjectProfile::from_project`) copies each default role's `skills` into `role_skills`, including skills that exist only in the source project's `.orrery/skills/`. A project created with `init --profile` then fails `orrery run` at `build_engine` with "skill '<name>' not found in .orrery/skills/ or the global skills directory" until the skill file is copied over. The README's profile section warns about MCP servers but not skills. Suggested fix: `profile save` prints a warning listing skills not found in the global skills dir (using `catalog_entries`, source `project`), and the README's "Global profiles" section says skills must be global or copied into the new project.
Found during: User Step LOOP-2 (cloud-cli profile names project-only `repo-rules`).

## Reproduction Steps
1. In a project with `.orrery/skills/repo-rules.md` (not global) and `coder.skills = ["repo-rules"]`: `orrery profile save p`.
2. In a fresh dir: `orrery init --profile p` (gate/hooks cleared), then `orrery run` → exit 1, "skill 'repo-rules' not found".

## Fix Log & Agent Notes
