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
    .description('[developer convenience] Run ./scripts/build.sh (for development only)')
    .argument('[args...]', 'Arguments passed to scripts/build.sh (e.g., server controller)')
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
