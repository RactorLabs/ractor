# Release

Automate the complete release workflow for Raworc project.

## What this command does:

1. **Stage all changes**: `git add .`
2. **Get current version**: Read version from `Cargo.toml`
3. **Commit changes**: Create commit with descriptive message
4. **Push to main**: `git push origin main`
5. **Tag release**: Tag current commit with project version
6. **Push tag**: `git push origin <version>` (triggers GitHub Actions)
7. **Bump version**: Increment patch version in all files:
   - `Cargo.toml`: Main project version
   - `cli/package.json`: CLI package version  
   - `CLAUDE.md`: Documentation version reference
8. **Commit version bump**: `git commit -m "chore: bump version to <next>"`
9. **Push version bump**: `git push origin main`

## Usage:

```bash
# From project root
claude release
```

## Example workflow:

```
Current version: 0.2.3
→ Stage changes and commit
→ Tag 0.2.3 and push (triggers GitHub Actions)
→ Bump to 0.2.4 for next development
→ Push version bump
```

This triggers the automated GitHub Actions workflow to:
- Build Docker images (raworc_*:0.2.3)
- Push to Docker Hub (raworc/raworc_*:0.2.3 + latest)  
- Publish npm package (@raworc/cli@0.2.3)
- Create GitHub release

## Version File Management

The release process updates version numbers in multiple files:
- **`Cargo.toml`**: Main Rust project version
- **`cli/package.json`**: npm CLI package version
- **`CLAUDE.md`**: Documentation version reference

All version references are automatically updated to maintain consistency across the codebase.

The project is then ready for the next development cycle at version 0.2.4.