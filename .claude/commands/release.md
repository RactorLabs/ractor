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
11. **Update website**: Deploy website with latest API docs, CLI changes, and version updates

**Next steps after release:**
- Use `/bump` command to increment version for next development cycle
- Use `/commit` command to commit the version bump changes

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
Current version: 0.4.4
→ Stage changes and commit
→ Tag 0.4.4 and push (triggers GitHub Actions) - NOTE: NO "v" prefix
→ Use /bump command to increment to 0.4.5
→ Use /commit to push version bump
```

## Version Management

After completing the release workflow, use the separate `/bump` command to increment the version for next development:

```bash
# After /release completes successfully:
/bump    # Increments version and updates all files
/commit  # Commits the version bump changes
```

The `/bump` command handles all version file updates and lock file generation automatically.
