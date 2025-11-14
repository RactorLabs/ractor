const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const COMPONENT_ALIASES = {
  a: 'api',
  c: 'controller',
  o: 'operator',
  s: 'sandbox',
};

function resolveComponentAlias(name = '') {
  const lower = name.toLowerCase();
  return COMPONENT_ALIASES[lower] || lower;
}

function normalizeArgs(args = []) {
  return args.map((arg) => (arg.startsWith('-') ? arg : resolveComponentAlias(arg)));
}

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
    .description('[development only] Rebuild TSBX components via ./scripts/rebuild.sh')
    .argument('[args...]', 'Components: api, controller, sandbox, operator, content, gateway. Shortcuts: a=api, c=controller, o=operator, s=sandbox. Flags are passed through.')
    .addHelpText('after', '\nAllowed components: api, controller, sandbox, operator, content, gateway\n' +
      'Shortcuts: a=api, c=controller, o=operator, s=sandbox\n' +
      '\nExamples:\n' +
      '  $ tsbx rebuild                    # rebuild all components (script default)\n' +
      '  $ tsbx rebuild controller         # rebuild controller\n' +
      '  $ tsbx rebuild api sandbox        # rebuild multiple components')
    .action(async (args = []) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'rebuild.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/rebuild.sh not found. This command is for development only.');
          process.exit(1);
        }
        const normalizedArgs = normalizeArgs(args);
        // Validate non-flag args are TSBX components
        const allowed = new Set(['api','controller','sandbox','operator','content','gateway']);
        const invalid = (normalizedArgs || []).filter(a => !a.startsWith('-')).filter(a => !allowed.has(a));
        if (invalid.length) {
          console.error(`[ERROR] Invalid component(s): ${invalid.join(', ')}. Allowed: api, controller, sandbox, operator, content, gateway`);
          process.exit(1);
        }
        await runScript(scriptPath, normalizedArgs);
      } catch (err) {
        console.error('[ERROR] rebuild failed:', err.message);
        process.exit(1);
      }
    });
};
