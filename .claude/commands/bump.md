# Bump

Increment or set the project version across all files and update lock files to prepare for the next development cycle.

## Usage

- `bump` - Bump to next patch version (0.3.7 → 0.3.8)
- `bump minor` - Bump to next minor version (0.3.7 → 0.4.0)
- `bump major` - Bump to next major version (0.3.7 → 1.0.0)
- `bump 0.5.2` - Set to specific version (0.3.7 → 0.5.2)

## What this command does

1. **Read current version**: Get current version from `Cargo.toml`
2. **Determine target version**: Either increment (patch/minor/major) or use specified version
3. **Validate version**: Ensure version format is valid (semver: X.Y.Z)
4. **Update all version references**: Update version in all 8 required files
5. **Build Rust project**: Run `cargo build --release` to validate and update `Cargo.lock`
6. **Update npm packages**: Run `npm install` in cli/ and website/ to update `package-lock.json` files
7. **Verify builds**: Ensure all builds succeeded

**Note**: This command only makes file changes. No commits are created. Use `/commit` after this command completes.

## Version Bump Requirements

**CRITICAL: When bumping versions, always build both Cargo and npm to ensure success and update lock files.**

### Files that must be updated (8 files total)

**Version References (manual updates):**

1. `Cargo.toml` - Main Rust project version
   ```toml
   version = "0.3.8"
   ```

2. `cli/package.json` - CLI npm package version
   ```json
   "version": "0.3.8"
   ```

3. `website/package.json` - Website package version  
   ```json
   "version": "0.3.8"
   ```

4. `CLAUDE.md` - Documentation version reference
   ```markdown
   Current version: 0.3.8
   ```

5. `src/server/rest/routes.rs` - API version response
   ```rust
   "version": "0.3.8"
   ```

6. `website/docs/api/rest-api.md` - API documentation version
   ```json
   "version": "0.3.8"
   ```

7. `website/docs/changelog.md` - Add new version entry placeholder
   ```markdown
   ## [0.3.8] - TBD
   
   Development version - changes will be documented at release.
   ```

8. `.claude/commands/release.md` - Update version examples
   ```
   Current version: 0.3.8 (in examples)
   ```

### Build Steps (updates lock files automatically)

**Rust Build:**
```bash
cargo build --release
```
- Validates new version doesn't break compilation
- Updates `Cargo.lock` with new version

**Node.js Packages:**
```bash
cd cli && npm install && cd ..
cd website && npm install && cd ..
```
- Validates package.json changes are correct
- Updates `cli/package-lock.json` and `website/package-lock.json`

### Lock Files (auto-updated by builds)

- `Cargo.lock` - Updated by `cargo build --release`
- `cli/package-lock.json` - Updated by `npm install` in cli/ folder  
- `website/package-lock.json` - Updated by `npm install` in website/ folder

## Version Increment Logic

**Semver Format**: All versions follow semantic versioning (MAJOR.MINOR.PATCH)

- **Patch**: Increment patch number, reset nothing (0.3.7 → 0.3.8)
- **Minor**: Increment minor number, reset patch to 0 (0.3.7 → 0.4.0)  
- **Major**: Increment major number, reset minor and patch to 0 (0.3.7 → 1.0.0)
- **Specific**: Set exact version as provided (0.3.7 → 0.5.2)

## Example Workflows

**Default patch increment:**
```
Current: 0.3.7 → Target: 0.3.8
→ Update all 8 version reference files
→ Run cargo build --release (updates Cargo.lock)
→ Run npm install in cli/ and website/ (updates package-lock.json files)
→ All 11 files ready for commit
```

**Minor version bump:**
```
bump minor
Current: 0.3.7 → Target: 0.4.0
→ Update all 8 version reference files
→ Build and validate
→ All 11 files ready for commit
```

**Specific version:**
```
bump 1.0.0
Current: 0.3.7 → Target: 1.0.0
→ Update all 8 version reference files  
→ Build and validate
→ All 11 files ready for commit
```

## Why This Process is Required

- **Rust validation**: `cargo build` ensures new version doesn't break compilation
- **Node.js validation**: `npm install` ensures package.json changes are valid  
- **Lock file updates**: `Cargo.lock` and `package-lock.json` must reflect version changes
- **Consistency**: Prevents version mismatches between source and lock files
- **Release reliability**: Ensures published packages will build successfully

## Verification

After running this command, verify all files were updated:

```bash
# Check git status to see all modified files
git status

# Should show 11 modified files:
# - 8 version reference files (manual updates)
# - 3 lock files (build updates)
```

All files should be ready for a single commit with the next version number.