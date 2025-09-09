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
    .description('[development only] Run ./scripts/rebuild.sh (fails if script missing)')
    .argument('[args...]', 'Arguments passed through to scripts/rebuild.sh (e.g., api controller)')
    .addHelpText('after', '\nExamples:\n  $ raworc rebuild                    # rebuild all components (script default)\n  $ raworc rebuild controller         # rebuild controller\n  $ raworc rebuild api agent          # rebuild multiple components')
    .action(async (args = []) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'rebuild.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/rebuild.sh not found. This command is for development only.');
          process.exit(1);
        }
        await runScript(scriptPath, args);
      } catch (err) {
        console.error('[ERROR] rebuild failed:', err.message);
        process.exit(1);
      }
    });
};
