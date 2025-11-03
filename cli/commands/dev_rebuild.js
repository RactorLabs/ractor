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
    .command('rebuild')
    .description('[development only] Rebuild TaskSandbox components via ./scripts/rebuild.sh')
    .argument('[args...]', 'Components: api, controller, session, operator, content, gateway. Flags are passed through.')
    .addHelpText('after', '\nAllowed components: api, controller, session, operator, content, gateway\n' +
      '\nExamples:\n' +
      '  $ tsbx rebuild                    # rebuild all components (script default)\n' +
      '  $ tsbx rebuild controller         # rebuild controller\n' +
      '  $ tsbx rebuild api session          # rebuild multiple components')
    .action(async (args = []) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'rebuild.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/rebuild.sh not found. This command is for development only.');
          process.exit(1);
        }
        // Validate non-flag args are TaskSandbox components
        const allowed = new Set(['api','controller','session','operator','content','gateway']);
        const invalid = (args || []).filter(a => !a.startsWith('-')).filter(a => !allowed.has(a));
        if (invalid.length) {
          console.error(`[ERROR] Invalid component(s): ${invalid.join(', ')}. Allowed: api, controller, session, operator, content, gateway`);
          process.exit(1);
        }
        await runScript(scriptPath, args);
      } catch (err) {
        console.error('[ERROR] rebuild failed:', err.message);
        process.exit(1);
      }
    });
};
