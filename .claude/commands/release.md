# Release

Automate the complete release workflow for Raworc project.

## What this command does

1. **Update documentation**: Update README (for developers), website (for end users), and CLAUDE.md (for yourself), if there are any changes to make
2. **Update changelog**: Add new version entry with all improvements from recent commits
3. **Stage documentation changes**: `git add .`
4. **Commit documentation**: Create commit for documentation updates
5. **Stage all remaining changes**: `git add .`
6. **Get current version**: Read version from `Cargo.toml`
7. **Commit changes**: Create commit with descriptive message
8. **Push to main**: `git push origin main`
9. **Tag release**: Tag current commit with project version (without "v" prefix)
10. **Push tag**: `git push origin <version>` (triggers GitHub Actions)

11. **Bump version**: Increment patch version in all files (see detailed list below)
12. **Run builds**: Update lock files and validate changes
13. **Stage all changes**: `git add .`
14. **Commit version bump**: `git commit -m "chore: bump version to <next>"`
15. **Push version bump**: `git push origin main`
16. **Update website**: Deploy website with latest API docs, CLI changes, and version updates

## Pre-Release Documentation Updates

**CRITICAL: Update all documentation before each release to reflect recent changes:**

### **Review Recent Commits**

```bash
# Review commits since last release to understand what changed
git log --oneline <last-version>..HEAD

# Look for features, fixes, and improvements to document
```

### **Update Documentation Files**

1. **README.md**: Update with recent features, fixes, and improvements
2. **website/docs/changelog.md**: Add comprehensive new version entry
3. **website/docs/getting-started.md**: Update with new features and workflows
4. **website/docs/concepts/**: Update concept docs if architecture changed
5. **website/src/components/HomepageFeatures/**: Update feature descriptions
6. **CLAUDE.md**: Update internal documentation with development changes

### **Key Areas to Update**

- **New features** from recent commits
- **Bug fixes** and reliability improvements  
- **CLI enhancements** and new commands
- **Session management** improvements
- **API changes** and new endpoints
- **Breaking changes** and migration notes

## Example workflow

```
Current version: 0.3.6
→ Stage changes and commit
→ Tag 0.3.6 and push (triggers GitHub Actions) - NOTE: NO "v" prefix
→ Bump to 0.3.7 for next development
→ Push version bump
```

## Version Bump Requirements

**CRITICAL: When bumping versions, always build both Cargo and npm to ensure success and update lock files:**

```bash
# 1. Update version in all files:
#    - Cargo.toml: version = "0.3.6"
#    - cli/package.json: "version": "0.3.6"  
#    - website/package.json: "version": "0.3.6"
#    - CLAUDE.md: Current version: 0.3.6
#    - src/server/rest/routes.rs: "version": "0.3.6" (API response)
#    - website/docs/api/rest-api.md: "version": "0.3.6" (documentation)
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
git commit -m "chore: bump version to 0.3.5"
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
