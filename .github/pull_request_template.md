<!--
Keep the summary in prose — what changed and why, written for a reviewer who hasn't
read the diff yet. Delete any section that doesn't apply, including its heading.
-->

## Summary



## Backlog task

<!-- e.g. TASK-109.2 — or "none" for a fix with no task. Task files are edited only
through the `backlog` CLI, and task updates ride the branch only when this PR does
the task's work. -->

## Checks

- [ ] `cargo fmt --check` passes (CI runs it first; a failure here fails the build)
- [ ] `cargo test -- --test-threads=1` passes (serial is not optional — the DB password is a process global)
- [ ] `./scripts/check-no-real-data.sh --staged` exits 0 — judged by exit status, never by grepping its output
- [ ] Web changes only: `npm test`, `npm run lint`, `npm run typecheck` pass from `web/`

## Visual changes

<!-- Delete this section for pure logic, state, or service work. -->

- [ ] The component lives in `web/packages/ui/src/components/` as `wc-*.ts` — no bespoke components in `apps/app`
- [ ] A co-located `*.preview.ts` covers the visible states, and `describePreviewA11y` passes with zero violations
- [ ] Tokens come from `@nigel/theme` — no inline brand values — and `controlsCss` is adopted wherever a `wa-*` primitive renders

## Documentation

<!-- Per the documentation policy: detail lands in the relevant docs/ file
(architecture, design-constraints, api, …), user-facing changes update README.md,
and CLAUDE.md changes only when a command, a rule, or a pointer changes. Say which
files this PR touched, or why none needed it. -->
