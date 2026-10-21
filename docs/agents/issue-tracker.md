# Issue tracker: GitHub

Issues and specifications for this repository live in GitHub Issues. Use `gh` from this repository.

## Routine operations

- Create: `gh issue create --title "..." --body-file <file>`
- Read: `gh issue view <number> --comments`
- List: `gh issue list --state open --json number,title,body,labels,comments`
- Comment: `gh issue comment <number> --body-file <file>`
- Apply or remove labels: `gh issue edit <number> --add-label "..."` or `--remove-label "..."`
- Close: `gh issue close <number>`

When a skill says "publish to the issue tracker", create a GitHub issue. When it says "fetch the relevant ticket", read the referenced issue.

## Pull requests as a triage surface

PRs as a request surface: no.

## Wayfinding

- A map is one issue labeled `wayfinder:map`; child tickets are GitHub sub-issues, or a task list in the map if sub-issues are unavailable.
- Child types use `wayfinder:<type>`: `research`, `prototype`, `grilling`, or `task`.
- Use native issue dependencies where available; otherwise record `Blocked by: #<number>` in the child.
- Claim an available child with `gh issue edit <number> --add-assignee @me`.
- Resolve it by commenting with the answer, closing it, and adding a short decision link to the map.
