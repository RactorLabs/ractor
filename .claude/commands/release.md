# Release

Automate the complete release workflow for Raworc project.

## What this command does

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
8. Run builds to update lock files

```bash
# Build Rust project to update Cargo.lock
cargo build --release

# Build npm package to update package-lock.json (if it exists)
cd cli && npm install && cd ..

# Verify both builds succeed before committing
```

9. **Stage all changes**: `git add .`
10. **Commit version bump**: `git commit -m "chore: bump version to <next>"`
11. **Push version bump**: `git push origin main`

## Example workflow

```
Current version: 0.2.3
→ Stage changes and commit
→ Tag 0.2.3 and push (triggers GitHub Actions)
→ Bump to 0.2.4 for next development
→ Push version bump
```

## Files to commit after every version bump

- `CLAUDE.md`: Documentation version reference
- `Cargo.toml`: Main Rust project version
- `Cargo.lock` reflects the new version
- `cli/package.json`: npm CLI package version
- `cli/package-lock.json` is updated (if present)
