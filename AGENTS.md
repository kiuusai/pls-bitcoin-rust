# AGENTS.md

This file defines working guidelines for AI coding agents in this repository

## Project overview
- Name: pls-bitcoin-lib (for Rust)
- Code type: Rust library
- Source code: `src/`
- Tests: `tests/`
- Testing enviroment: `.devcontainer/`

## Primary goals for agents
1. Make minimal, focused changes that solves the user request
2. Preserve existing architecture, naming and style conventions
3. Keep multisig execution safe avoiding key leaks, bad signatures, signing attacks and other similar security issues
4. Consider that users will share signatures with each other asynchronous to unlock funds
5. Validate changes with relevant tests
6. Users should be able to securely creates multisigs that exactly matches with ones created with older code version
7. Document each public resource with a short explanation about their usability and if needed, some warning about security checks
8. Search for any possible security issue inside current code and report it immediately

## Safety rules
- Ensure user cannot expose their own key (directly and indirectly via signing checks or reverse engineering)
- No logs on `src/` are allowed. Just on `tests` for debug purposes
- Any code change should pass in E2E tests
- Any logic config should be explicitly added on data structs
- Signatures NEVER can reveal secrets based on nonce comparison or any other known analysis
- Avoid destructive git commands unless explicitly requested

## Editing guidelines
- Prefer small diffs over broad refactors
- Prefer define variable type at start instead of on method execuction in cases it's required static typing
- Use shortest path for variable definitions every time it's possible
- Match existing coding patterns before introducing new abstractions
- Add brief comments only for non obviously logic
- Update docs when behavior, flows, or developer commands change

## Testing expectations
- Any resolved security issue should generate a test that ensures it's resolved
- Tests are only changed where explicitly code logic is modified
- Tests cannot bypass current code logic flow unless explicitly required
- If specific test doesn't exists, create one that satisfies the functionality necessity
- Avoid adding environment variables. If needed, put a coherent default value on this
- Use rstest to create test cases and resources if needed

Suggested commands (run from repository root):
```bash
cargo test -- --show-output
```

## Regtest and local infra
- Use `nigiri-rs` tooling to reproduce Bitcoin transactions
- Tests should successfully runs inside a fresh devcontainer install

## Workflow for agents
1. Read the request and inspect only relevant files
2. Propose or apply minimal changes
3. Summarize what changed, validation performed and residual risks
4. Files described in .gitignore should be completely ignored

## Commit guidance
- Keep commits atomic and descriptive
- Group related code, tests and docs in the same commit
- Avoid mixing unrelated cleanups with functional changes

## Definition of done
A tasks is completed when:
1. The requested behavior is implemented
2. Relevant checks/tests pass or failures are explained and resolved
3. No obvious regression are introduced in nearby flows
4. Documentation is updated when needed
