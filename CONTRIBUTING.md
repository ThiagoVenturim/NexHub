# Contributing to NexHub

Thank you for contributing to NexHub. Please adhere to the following standards:

## Repository Standards

1. No Emojis: All code, documentation, commit messages, and UI strings must remain free of emoji. Run `python scripts/check_no_emoji.py` before committing.
2. No AI Co-Authorship Trailers: Do not include "Co-Authored-By", "Generated with", or similar trailers in commit messages or pull requests.
3. Git Hooks: Configure hooks on your clone by running:
   ```bash
   git config core.hooksPath .githooks
   ```
4. Conventional Commits: Use short, conventional commit messages in English (`feat: ...`, `fix: ...`, `docs: ...`, `chore: ...`).
5. Code Verification: Ensure all checks pass before submitting pull requests:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace -- -D warnings`
   - `cargo test --workspace`
   - `npm --prefix ui test`
   - `python scripts/check_no_emoji.py`
