const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

function runScript(script, args = []) {
  return new Promise((resolve, reject) => {
    const p = spawn('bash', [script, ...args], { stdio: 'inherit', shell: false });
    p.on('exit', (code) => code === 0 ? resolve() : reject(new Error(`${script} failed with code ${code}`)));
    p.on('error', reject);
  });
}

module.exports = (program) => {
  program
    .command('build')
    .description('[development only] Build Ractor images via ./scripts/build.sh')
    .argument('[args...]', 'Components: api, controller, agent, operator, content, gateway, app_githex, app_askrepo (apps are opt-in). Flags are passed through (e.g., -n, --no-cache).')
    .addHelpText('after', '\nAllowed components: api, controller, agent, operator, content, gateway, app_githex, app_askrepo\n' +
      '\nExamples:\n' +
      '  $ ractor build                       # builds all (script default)\n' +
      '  $ ractor build api controller        # build only api and controller\n' +
      '  $ ractor build operator content      # build Operator UI and Content\n' +
      '  $ ractor build app_githex            # build the GitHex app image\n' +
      '  $ ractor build app_askrepo           # build the AskRepo app image\n' +
      '  $ ractor build -- -n --no-cache      # pass flags through to script')
    .action(async (args = []) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'build.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/build.sh not found. This command is for development only.');
          process.exit(1);
        }
        // Validate non-flag args are Ractor components (or 'all')
        const allowed = new Set(['api','controller','agent','operator','content','gateway','app_githex','app_askrepo','all']);
        const invalid = (args || []).filter(a => !a.startsWith('-')).filter(a => !allowed.has(a));
        if (invalid.length) {
          console.error(`[ERROR] Invalid component(s): ${invalid.join(', ')}. Allowed: api, controller, agent, operator, content, gateway`);
          process.exit(1);
        }
        await runScript(scriptPath, args);
      } catch (err) {
        console.error('[ERROR] build failed:', err.message);
        process.exit(1);
      }
    });
};
