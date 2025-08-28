# Release

Automate the complete release workflow for Raworc project.

## What this command does

1. **Stage all changes**: `git add .`
2. **Get current version**: Read version from `Cargo.toml`
3. **Commit changes**: Create commit with descriptive message
4. **Push to main**: `git push origin main`
5. **Tag release**: Tag current commit with project version (without "v" prefix)
6. **Push tag**: `git push origin <version>` (triggers GitHub Actions)

7. **Bump version**: Increment patch version in all files (see detailed list below)
8. **Run builds**: Update lock files and validate changes
9. **Stage all changes**: `git add .`
10. **Commit version bump**: `git commit -m "chore: bump version to <next>"`
11. **Push version bump**: `git push origin main`
12. **Update website**: Deploy website with latest API docs, CLI changes, and version updates

## Example workflow

```
Current version: 0.2.7
→ Stage changes and commit
→ Tag 0.2.7 and push (triggers GitHub Actions) - NOTE: NO "v" prefix
→ Bump to 0.2.8 for next development
→ Push version bump
```

## Version Bump Requirements

**CRITICAL: When bumping versions, always build both Cargo and npm to ensure success and update lock files:**

```bash
# 1. Update version in all files:
#    - Cargo.toml: version = "0.2.8"
#    - cli/package.json: "version": "0.2.8"  
#    - website/package.json: "version": "0.2.8"
#    - CLAUDE.md: Current version: 0.2.8
#    - src/server/rest/routes.rs: "version": "0.2.8" (API response)
#    - website/docs/api/rest-api.md: "version": "0.2.8" (documentation)
#    - website/docs/changelog.md: Add new version entry with changes
#    - .claude/commands/release.md: Update version examples

# 2. Build Rust project to validate and update Cargo.lock
cargo build --release

# 3. Build npm packages to validate and update package-lock.json (if exists)
cd cli && npm install && cd ..
cd website && npm install && cd ..

# 4. Verify both builds succeeded before committing
# 5. Commit all changes including updated lock files
git add Cargo.toml cli/package.json website/package.json CLAUDE.md \
        src/server/rest/routes.rs website/docs/api/rest-api.md \
        website/docs/changelog.md .claude/commands/release.md \
        Cargo.lock cli/package-lock.json website/package-lock.json
git commit -m "chore: bump version to 0.2.8"
```

**Why this is required:**
- **Rust validation**: `cargo build` ensures new version doesn't break compilation
- **Node.js validation**: `npm install` ensures package.json changes are valid
- **Lock file updates**: `Cargo.lock` and `package-lock.json` must reflect version changes
- **Consistency**: Prevents version mismatches between source and lock files
- **Release reliability**: Ensures published packages will build successfully

### Files that must be updated and committed with version bumps

**Version References (manual updates):**
- `Cargo.toml` - Main Rust project version
- `cli/package.json` - CLI npm package version
- `website/package.json` - Website package version
- `CLAUDE.md` - Documentation version reference
- `src/server/rest/routes.rs` - API version response
- `website/docs/api/rest-api.md` - API documentation version
- `website/docs/changelog.md` - Version changelog entry
- `.claude/commands/release.md` - Release workflow examples

**Lock Files (auto-updated):**
- `Cargo.lock` - Updated by `cargo build --release`
- `cli/package-lock.json` - Updated by `npm install` in cli/ folder
- `website/package-lock.json` - Updated by `npm install` in website/ folder

## Post-Release Website Update

After completing the release, update the website to reflect all changes:

```bash
# Build and deploy website with latest changes
cd website
npm run build
npm run deploy  # or your deployment method

# Verify website shows:
# - Updated API documentation with new version
# - Latest changelog entries
# - Current CLI commands and examples
# - Updated version references throughout
```

**Website Update Checklist:**
- [ ] API documentation reflects new version in examples
- [ ] Changelog shows latest release with feature descriptions
- [ ] CLI usage examples are current and accurate
- [ ] All version references match released version
- [ ] New features are documented with examples
- [ ] Troubleshooting guides are up to date
