# Bump

Increment the project version across all files and update lock files to prepare for the next development cycle.

## What this command does

1. **Read current version**: Get current version from `Cargo.toml`
2. **Increment version**: Bump patch version (e.g., 0.3.7 → 0.3.8)
3. **Update all version references**: Update version in all 8 required files
4. **Build Rust project**: Run `cargo build --release` to validate and update `Cargo.lock`
5. **Update npm packages**: Run `npm install` in cli/ and website/ to update `package-lock.json` files
6. **Verify builds**: Ensure all builds succeeded

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

## Example Workflow

```
Current version in Cargo.toml: 0.3.7
→ Increment to 0.3.8
→ Update all 8 version reference files
→ Run cargo build --release (updates Cargo.lock)
→ Run npm install in cli/ and website/ (updates package-lock.json files)
→ All 11 files ready for commit (8 manual + 3 lock files)
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