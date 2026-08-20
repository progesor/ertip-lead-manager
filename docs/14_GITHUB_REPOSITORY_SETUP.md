# 14 — GitHub Repository Setup

## Recommended repository name

`ertip-lead-manager`

The name is only a working recommendation; renaming does not change the product canon.

## Initial repository state

The first commit should contain only the documentation/bootstrap files from this package. Production lead exports must not be included.

Suggested first commit message:

```text
docs: bootstrap Ertip Lead Manager canon and roadmap
```

## Option A — GitHub Desktop

1. Create a new local repository named `ertip-lead-manager`.
2. Copy the contents of this package into the repository root.
3. Review `.gitignore` before adding any other files.
4. Commit all documentation with the suggested initial commit message.
5. Publish the repository to GitHub.
6. Keep the repository private initially because the application domain involves customer lead data, even though real PII must never be committed.

## Option B — Git CLI

From the package/repository root:

```bash
git init
git add .
git commit -m "docs: bootstrap Ertip Lead Manager canon and roadmap"
git branch -M main
git remote add origin <YOUR_REPOSITORY_URL>
git push -u origin main
```

## Branch policy

For the current team size, keep it simple:

- `main` is canonical and should build/pass tests after code begins.
- Use short-lived branches such as `m1/foundation`, `m2/import-preview`, or focused feature branches.
- Merge through PRs when useful for review/history.

## GitHub labels (optional)

Suggested labels:

- `milestone:m0`
- `milestone:m1`
- `milestone:m2`
- `milestone:m3`
- `milestone:m4`
- `milestone:m5`
- `milestone:m6`
- `area:import`
- `area:data`
- `area:ui`
- `area:analytics`
- `area:desktop`
- `bug`
- `docs`
- `tech-debt`

Do not spend significant time configuring project-management metadata before M1.

## After publishing

Start the development conversation/agent with `REPO_START_PROMPT.md` and the GitHub repository link.

The implementation agent should inspect the repository first, complete M0, then begin M1 only after canon conflicts are resolved.
