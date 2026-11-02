# Linear and GitHub workflow

This document defines how GLUE 5 contributors and AI agents coordinate work. Read it together with [AI_GUIDE.md](../AI_GUIDE.md) before starting a repository task.

## Two systems, two responsibilities

**Linear is the work tracker and coordination record.** Use the GLUE 5 team and project to track task scope, priorities, ownership, status, acceptance criteria, decisions, blockers, and handoffs. Linear tells collaborators what is intended, why it matters, who is working on it, and what remains.

**GitHub is the implementation source of truth.** The repository, branches, commits, pull requests, workflow definitions, CI runs, and actual files determine what code and documentation exist. Always verify implementation claims against the relevant branch and commit in GitHub. A Linear issue saying that something is implemented does not prove that the change was committed, pushed, or tested.

| Question | Use |
|---|---|
| What needs to be done, why, and at what priority? | Linear |
| Who owns it, what is blocked, and what is the next handoff? | Linear |
| What decision or acceptance criteria were agreed? | Linear, with durable technical decisions also documented in the repository |
| What code exists on a branch or commit? | GitHub |
| Did a workflow or test actually pass? | GitHub Actions run and its logs |
| Was a change published or integrated? | GitHub branch/PR/commit history |

## Workspace and project

- Team: **Glue5** (key: `GLU`).
- Project: **GLUE 5 — GTA V para Android** (identifier: `P-GLU-1`).
- Create new project work in this team and project unless the owner explicitly directs otherwise.
- Search for an existing issue before creating a new one. Avoid duplicate issues for the same task.
- Use a short area prefix in issue titles, for example `[MENU]`, `[TOUCH]`, `[SERV]`, `[PERF]`, or `[CI]`. The prefix identifies the responsible area/chat, not the AI vendor.

## Start a task

1. Read [AI_GUIDE.md](../AI_GUIDE.md), this document, and the task-specific repository docs.
2. Inspect the actual checkout: `git status --short --branch`, recent commits, diffs, and the assigned branch. Preserve pre-existing and untracked work.
3. Search Linear for the task or relevant existing issue. Read its full description, comments, linked documents, acceptance criteria, and current status. Do not treat a search-result summary as the full specification.
4. Confirm that the current branch contains the implementation you are about to change. Another agent's Linear update does not imply its branch has been merged into yours.
5. If no matching issue exists and the work is independently actionable, create one in the GLU team and P-GLU-1 project. Keep the title specific and use the area's short prefix.
6. Move the issue to **In Progress** when active implementation begins. Do not change status merely because the issue was read.

## Keep Linear current while working

- Record material decisions, changed scope, dependencies, test evidence, and handoff details in the relevant issue. Prefer concise, additive comments over repeatedly replacing the whole description.
- When blocked, write the exact blocker, its evidence, and the action or access needed to unblock it. Do not invent a `Blocked` state if the team's workflow does not provide one; keep the actual status accurate.
- Before handing work to another agent, record the branch name, commit SHA (if one exists), changed paths, checks run, checks not run, and what the next agent should do.
- Keep implementation facts and task status separate. For example, “code written locally,” “committed,” “pushed,” “CI passed,” and “validated on the owner's Android device” are different states.
- Never claim a build, test, push, review, merge, or device validation succeeded unless the corresponding result was observed.
- Do not store secrets, tokens, private keys, proprietary game files, or unredacted personal data in Linear. The repository must not contain GTA V game assets.

## Completion and integration

- Update the issue with links to the relevant commit, pull request, and CI run, plus a short summary of the actual result.
- Use **In Review** when implementation is ready for review or an external validation gate remains. Use **Done** only after the issue's acceptance criteria are satisfied and required validation is complete.
- If the owner must run the Android build or validate on the device, leave that gate visibly pending; do not mark the task Done just because local checks passed.
- A task being Done in Linear does not authorize merging or releasing. Follow the branch ownership and integration rules in [AI_GUIDE.md](../AI_GUIDE.md). Never force-push.

## Required identification for AI contributions

Every AI-authored issue description, comment, or status/update text in Linear must start with its own identification line:

`[IA: <nome do modelo> | Agente/cliente: <ferramenta e contexto do agente/chat>]`

Example:

`[IA: GPT-6 | Agente/cliente: ChatGPT]`

Each agent identifies its own contribution, even when another agent already contributed to the same issue. Preserve other authors' text and identifiers. Do not attribute work by another agent to yourself.

When an AI's work is incorporated into a Git commit, include this trailer in the commit message:

`AI-Contribution: <modelo> | Agente/cliente: <ferramenta>`

Example:

`AI-Contribution: GPT-6 | Agente/cliente: ChatGPT`

Keep Git's normal author and committer fields intact. The trailer identifies the AI contribution; it does not replace or falsify authorship. If multiple AIs materially contributed, add a separate trailer for each.

## Codename and project identity

The current project codename is **`willow`**. Keep codename, semantic version, and release state independent. A codename is one short English word (preferably 4–8 letters); change it only for a major change in architecture, overall experience, or technical direction. Do not change it for routine fixes, isolated features, build retries, or release-state changes. When a codename change is approved, record the date, old and new names, and reason in Linear and durable repository documentation.

## If an integration is unavailable

Do not silently skip coordination or pretend that a remote change was published. Record the exact missing permission, tool failure, branch, and local/remote state in the relevant issue. Keep local work clearly distinguished from pushed work, and ask the owner for the specific access or decision needed.
