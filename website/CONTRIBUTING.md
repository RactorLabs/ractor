# Contributing to Raworc

Thank you for your interest in contributing to Raworc! This guide outlines our standards for commit messages, pull requests, and development practices.

## Commit Message Format

Use the conventional commit format with clear, concise descriptions:

```
type: brief description

Optional longer description explaining the change.
```

### Commit Types

- `feat`: New feature or functionality
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring without feature changes
- `test`: Adding or updating tests
- `chore`: Maintenance tasks, dependencies, build changes
- `perf`: Performance improvements
- `style`: Code style changes (formatting, whitespace)

### Examples

```
feat: add session pause/resume functionality
fix: resolve container cleanup race condition
refactor: extract authentication middleware
test: add integration tests for agent deployment
chore: update Docker base images to latest
```

### Commit Guidelines

- Keep the subject line under 50 characters
- Use imperative mood ("add" not "added" or "adds")
- Do not end the subject line with a period
- Separate subject from body with a blank line
- Wrap the body at 72 characters
- Focus on what and why, not how

## Pull Request Format

### Title

Use the same format as commit messages:

```
type: brief description of changes
```

### Description Template

```markdown
## Summary

- Brief bullet point of key changes
- Another key change
- Third key change

## Test Plan

- [ ] Manual integration testing completed
- [ ] Changes verified in development environment
- [ ] Documentation updated if needed

## Breaking Changes

List any breaking changes and migration steps if applicable.

## Additional Notes

Any additional context, screenshots, or considerations for reviewers.
```

## Development Guidelines

### Code Style

- Follow existing code patterns and conventions
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings

### Testing

- Manually test integration with existing features
- Test edge cases and error conditions
- Verify changes work as expected in development environment

### Documentation

- Update README.md for user-facing changes
- Add inline documentation for complex code
- Update API documentation for endpoint changes
- Include examples in documentation when helpful

## Branch Naming

Use descriptive branch names that match the type of work:

```
type/brief-description
```

### Examples

```
feat/session-pause-resume
fix/container-cleanup-race
docs/api-reference-update
refactor/auth-middleware
chore/update-docker-images
```

### Guidelines

- Use lowercase with hyphens for separation
- Keep names concise but descriptive
- Match the commit type that will be used
- Avoid special characters except hyphens

## Submitting Changes

1. Fork the repository
2. Create a feature branch from `main` using the naming convention above
3. Make your changes following the guidelines above
4. Test your changes thoroughly
5. Submit a pull request with the proper format

## Getting Help

- Check existing issues and documentation first
- Open an issue for bugs or feature requests
- Join discussions in existing issues
- Ask questions in pull request comments

Thank you for contributing to Raworc!

