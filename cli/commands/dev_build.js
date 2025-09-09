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
    .description('[development only] Run ./scripts/build.sh (fails if script missing)')
    .argument('[args...]', 'Arguments passed through to scripts/build.sh (e.g., server controller)')
    .addHelpText('after', '\nExamples:\n  $ raworc build                       # builds all (script default)\n  $ raworc build server controller     # pass components to script\n  $ raworc build -- -n --no-cache      # pass flags through to script')
    .action(async (args = []) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'build.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/build.sh not found. This command is for development only.');
          process.exit(1);
        }
        await runScript(scriptPath, args);
      } catch (err) {
        console.error('[ERROR] build failed:', err.message);
        process.exit(1);
      }
    });
};
