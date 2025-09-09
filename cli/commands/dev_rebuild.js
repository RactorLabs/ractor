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
    .description('[developer convenience] Run ./scripts/rebuild.sh (for development only)')
    .argument('<components...>', 'Components to rebuild (server, controller, agent, operator, content, gateway)')
    .action(async (components) => {
      try {
        const scriptPath = path.join(process.cwd(), 'scripts', 'rebuild.sh');
        if (!fs.existsSync(scriptPath)) {
          console.error('[ERROR] scripts/rebuild.sh not found. This command is for development only.');
          process.exit(1);
        }
        await runScript(scriptPath, components);
      } catch (err) {
        console.error('[ERROR] rebuild failed:', err.message);
        process.exit(1);
      }
    });
};
