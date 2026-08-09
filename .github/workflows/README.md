# Workflows

CI runs on pull requests targeting main and pushes to main. It checks:

- Rust formatting, Core tests and Core Clippy with all features;
- Desktop frontend tests and production build;
- JSON contract syntax;
- repository secret patterns without printing matched values.

Workflow permissions are read-only. Third-party Actions are limited to official GitHub Actions and pinned to full commit SHAs. CI does not deploy, publish artifacts or access production secrets.
