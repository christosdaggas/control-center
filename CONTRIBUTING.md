# Contributing to Control Center

Thank you for your interest in contributing! This document outlines the guidelines and processes for contributing to the project.

## Code Quality Standards

### Formatting and Linting

- **rustfmt**: All code must be formatted with `rustfmt`
- **clippy**: All clippy warnings must be addressed
  ```bash
  cargo fmt --check
  cargo clippy -- -D warnings
  ```

### Code Style

1. **Clarity over cleverness**: Prefer readable code
2. **Small, composable functions**: Keep functions focused
3. **Early returns**: Avoid deep nesting
4. **Explicit types**: Use explicit types for public APIs
5. **Meaningful names**: Names should reflect intent

### Error Handling

- Use `Result` with structured error types
- Use `thiserror` for error enums
- No panics in production paths
- Validate inputs at boundaries

### Testing

Run tests before submitting:
```bash
cargo test
```

## Duplication Prevention Checklist

Before submitting a PR, review your changes:

- [ ] Are there any functions that do similar things? → Refactor into shared function
- [ ] Are there repeated patterns in UI code? → Extract into reusable widgets
- [ ] Are there similar structs? → Consider using generics or traits
- [ ] Are there copy-pasted code blocks? → Parameterize into a function

## Architecture Rules

1. **Domain layer is pure**: No imports from UI or infrastructure crates
2. **Adapters implement traits**: All system integrations use trait abstraction
3. **State flows one direction**: Use actions and state updates, not direct mutation
4. **Evidence on all events**: Every event should reference its source data

## Icon Guidelines

When adding new event types or UI elements:

1. Use Freedesktop standard icon names
2. Add fallback names to `IconResolver`
3. Never hardcode icon paths
4. Test with both light and dark themes

## Cross-Desktop Testing

Before major releases, test on:

- [ ] Fedora GNOME (Wayland)
- [ ] Fedora KDE Plasma
- [ ] Ubuntu GNOME
- [ ] COSMIC (if available)

## Commit Messages

Use conventional commits:
- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
