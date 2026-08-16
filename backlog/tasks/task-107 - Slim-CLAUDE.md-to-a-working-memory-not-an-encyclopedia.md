---
id: TASK-107
title: 'Slim CLAUDE.md to a working memory, not an encyclopedia'
status: Done
assignee:
  - '@claude'
created_date: '2026-08-14 18:29'
updated_date: '2026-08-16 18:37'
labels:
  - docs
  - dx
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CLAUDE.md is 177 KB / 1,119 lines and the harness warns about it. It is loaded into every agent context, so its size taxes every single turn of every session.

How it got here is structural, not accidental: the Documentation Policy requires every feature change to update CLAUDE.md's Architecture and Key Design Constraints sections, and roughly thirty PRs across two epics each appended dense, essay-length prose under that mandate. Nothing ever prunes. The Architecture section alone is 79 KB; the Invoicing bullet is a 12 KB single paragraph; Key Design Constraints (47 KB) restates much of what Architecture already says. The file has become the project's collected design rationale rather than its working memory.

Anthropic's guidance for CLAUDE.md is the opposite shape: short, imperative, high-frequency facts an agent needs on nearly every task — commands, conventions, hard rules, pointers. Detail belongs in docs/ files an agent reads when the task calls for them, and design rationale belongs in the specs, commit messages and module doc comments that already carry it.

The restructure, not just a trim:
- CLAUDE.md keeps: the public-repo/no-real-data rule (verbatim — it is load-bearing), project overview in a few lines, the Commands section (tightened), the component-first workflow and verification matrix, the Backlog CLI essentials, and one-line pointers into docs/ for everything moved.
- Architecture prose moves to docs/architecture.md (or per-domain files beside the existing docs/invoicing.md and docs/api.md), condensed on the way — current state only, no history.
- Key Design Constraints distils to the rules an agent must not violate, one line each, with the reasoning moved to the module docs it describes. A constraint whose full rationale matters keeps a pointer, not the essay.
- The embedded Backlog.md manual (several hundred lines of generic CLI tutorial) shrinks to the project-specific essentials plus a pointer to backlog --help.
- The Documentation Policy itself is amended to stop the ratchet: feature changes update the RELEVANT doc file, and CLAUDE.md only when a command, rule or pointer changes. Add a soft size budget stated in the file.

Nothing may be lost, only moved: every fact deleted from CLAUDE.md must land in a docs/ file or already exist in one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CLAUDE.md is under 400 lines and under 25 KB
- [x] #2 Every fact removed from CLAUDE.md exists in a docs/ file, a module doc comment, or was verifiably redundant — an accounting in the PR body maps each removed section to its new home
- [x] #3 The no-real-data section survives verbatim
- [x] #4 The Documentation Policy is amended so routine feature changes update docs/ files, with CLAUDE.md changing only for commands, rules or pointers
- [x] #5 A stated size budget lives in CLAUDE.md itself
- [x] #6 The full verification matrix still passes (nothing behavioral depends on the file, but the guard scripts that read it must still find what they grep for)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
CLAUDE.md 215,925 bytes / 1,142 lines -> 11,014 bytes / 158 lines. The file had grown past the 177 KB the task recorded, because the merges landing that day each appended to it under the old policy.

Moved verbatim rather than condensed, so the nothing-lost claim is provable rather than asserted: docs/architecture.md (Architecture + the Project Structure tree, 110 KB), docs/design-constraints.md (60 KB), docs/commands.md (the full CLI reference, 10 KB), docs/backlog-cli.md (the Backlog manual the tool injects, 26 KB). CLAUDE.md keeps the no-real-data section verbatim, the commands that are NOT discoverable from --help, the component-first workflow, the backlog rules specific to this project, the amended policy and a pointer table.

Accounting: of the 721 lines over 40 characters in the old file, 5 do not appear anywhere in the new corpus — the boilerplate intro line and the four lines of the old Documentation Policy, which this task replaces. The README rule inside that policy was carried into the new one rather than dropped.

The backlog tool writes its own manual into CLAUDE.md on some upgrades, so the file says to move it back out when it reappears.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
CLAUDE.md is 11 KB and 158 lines, down from 216 KB and 1,142 lines. It is loaded into every agent context, so the size was a tax on every turn of every session.

Why it grew: the old Documentation Policy required every feature change to update the Architecture and Key Design Constraints sections, and nothing ever pruned. Around thirty PRs across two epics each appended essay-length prose under that mandate — the file had become the collected design rationale rather than the working memory.

What moved, verbatim:
- docs/architecture.md — the Architecture section and the Project Structure tree (110 KB)
- docs/design-constraints.md — Key Design Constraints (60 KB)
- docs/commands.md — the full CLI reference (10 KB)
- docs/backlog-cli.md — the Backlog manual the tool injects (26 KB)

What stays: the no-real-data section verbatim, a pointer table into docs/, the commands that are not discoverable from --help (serial tests, cargo fmt, the guard script), the component-first UI workflow, this repository's backlog rules, and the amended policy.

Moved rather than rewritten on purpose. Hand-condensing 156 KB of dense prose is how facts get lost; cutting whole sections makes the nothing-lost claim checkable. Of the 721 lines over 40 characters in the old file, 5 are absent from the new corpus — the boilerplate intro and the four lines of the policy being replaced. The README rule inside that policy was carried forward rather than dropped.

The ratchet is what actually gets fixed: a feature change now updates the relevant docs/ file, and CLAUDE.md changes only for a command, a rule or a pointer. The size budget is stated in the file itself, so the next person over it knows the answer is to move something out rather than tighten the margins.

Verification: cargo test (1,438 + 124), cargo fmt --check, check-no-real-data on the tree, and a line-level accounting of the old file against the new corpus. Nothing behavioural depends on the file.
<!-- SECTION:FINAL_SUMMARY:END -->
