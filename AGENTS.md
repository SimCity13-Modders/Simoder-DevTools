# Repository rules

- Developer executables, authoring toolchains, IDE support and example mods belong to this repository; do not embed them into the Simoder runtime distribution.
- Put exploratory analyses, audits and raw investigation reports in ignored `.analysis/`. Maintain public guides and API contracts in `docs/`.
- Use Doxygen-style code comments with `@summary` and relevant parameter/return annotations. Do not leave TODO placeholders.
- Preserve local changes and do not push or publish without explicit user instruction.
- Prefix shell commands with `rtk`; follow `C:/Users/dawid/.codex/RTK.md`.

## Delivery workflow

- Work in large approved stages (for example U4/U5). Keep one stage STATUS/decisions record, one final REPORT and independent final audit; internal fixes do not need separate dossiers or acceptance gates.
- Compile and run approved local regressions early, before implementation audit. Batch independent cases; any required failure still makes the overall result FAIL. Preserve raw failures and NOT_RUN evidence.
- Reuse qualified runners, toolchains and security profiles. Review changed effects or risks, not an unchanged procedure for each source fix or new run ID.
- Significant architecture changes require user approval. Make routine design and contract decisions within authorized scope; review material risks proportionately and obtain any required approval for privileged, destructive or external effects.
- Maintain API/architecture/user documentation with behavior at every size. Use the [shared lifecycle](../Simoder-Engineering/docs/ai/lifecycle.md) when available; it does not grant extra permissions.

## Continuous implementation and final audit

- Work autonomously in large coherent increments until the requested objective is implemented. Continue within the same active task across internal fixes and context continuations; do not end a turn merely to report progress or because an investigation is difficult. Send concise progress updates while work continues.
- Integrate implementation and migration in the owning repositories. Use focused checks when needed to unblock or assess changed risk; defer the full applicable build, regression suite and independent implementation audit until the large implementation boundary. An audit is not a substitute for completing implementation.
- Continue authorized independent work when part of the task needs input or is blocked. Stop only for an explicit stop, a genuine blocker that prevents further meaningful work, or a required decision or permission; preserve state and report the specific boundary. Never bypass access restrictions or rejected approvals.
- Preserve failures and unverified outcomes. After implementation, run the applicable integrated verification and arrange an independent final audit; address findings and repeat affected checks. Human acceptance and explicit permissions remain in force where required.

## Finish the requested objective

- Continue the same authorized task through implementation, integration, fixes and required verification until the user's full objective is met. A status update, a small successful change, elapsed time, context continuation, difficult investigation, individual test failure or completion of one component is not a reason to end work.
- If one path is blocked or awaits input or approval, ask only for what is necessary and continue all other meaningful authorized work. An approval rejection applies to that action: do not bypass it, and pursue independent work. Stop only for an explicit user stop or when no meaningful authorized action remains without the required input, permission or external change.
- Judge completion against the original requirements and actual integration evidence. Do not replace the requested end state with a narrower prototype, isolated fixture, passing component test or report. Preserve failures and distinguish unverified behavior.
- Use progress messages during ongoing work; a final response is not a substitute for continuing. Existing higher-priority instructions, significant architecture approval, security boundaries and external-action permissions remain in force.
