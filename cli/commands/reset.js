const chalk = require('chalk');
const { spawn } = require('child_process');
const readline = require('readline');
const display = require('../lib/display');

async function execDocker(args, options = {}) {
  return new Promise((resolve, reject) => {
    const docker = spawn('docker', args, { stdio: options.silent ? 'pipe' : 'inherit', ...options });
    let stdout = '';
    let stderr = '';
    if (options.silent) {
      docker.stdout.on('data', (d) => (stdout += d.toString()));
      docker.stderr.on('data', (d) => (stderr += d.toString()));
    }
    docker.on('exit', (code) => code === 0 ? resolve({ stdout, stderr }) : reject(new Error(stderr || 'Docker command failed')));
    docker.on('error', reject);
  });
}

module.exports = (program) => {
  program
    .command('reset')
    .description('Reset Docker: remove ALL containers, images, volumes, networks (destructive)')
    .option('-y, --yes', 'Confirm without prompting (non-interactive)')
    .addHelpText('after', '\n' +
      'WARNING: This resets your entire Docker environment, not just TaskSandbox.\n' +
      '\nExamples:\n' +
      '  $ tsbx reset           # interactive confirmation\n' +
      '  $ tsbx reset -y       # non-interactive\n')
    .action(async (options) => {
      try {
        display.showCommandBox(`${display.icons.reset} Docker Reset`, { operation: 'ALL containers, images, volumes, networks' });

        if (!options.yes) {
          const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
          const answer = await new Promise((resolve) => rl.question(chalk.yellow('This will remove ALL Docker containers/images/volumes/networks. Continue? [y/N]: '), resolve));
          rl.close();
          if (!answer.match(/^[Yy]$/)) { display.info('Operation cancelled'); return; }
        }

        // Stop and remove ALL containers
        display.info('[1/5] Stopping and removing ALL containers...');
        try {
          const resRun = await execDocker(['ps','-q'], { silent: true });
          const running = (resRun.stdout || '').trim().split('\n').filter(Boolean);
          if (running.length) { try { await execDocker(['stop', ...running], { silent: true }); } catch(_) {} }
          const resAll = await execDocker(['ps','-a','-q'], { silent: true });
          const all = (resAll.stdout || '').trim().split('\n').filter(Boolean);
          if (all.length) {
            await execDocker(['rm','-f', ...all], { silent: true });
            display.success(`Removed ${all.length} containers`);
          } else {
            display.success('No containers found');
          }
        } catch (e) { display.warning(`Container cleanup warning: ${e.message}`); }

        // Remove ALL images
        display.info('[2/5] Removing ALL images...');
        try {
          const ir = await execDocker(['images','-q'], { silent: true });
          const ids = (ir.stdout || '').trim().split('\n').filter(Boolean);
          if (ids.length) { try { await execDocker(['rmi','-f', ...ids], { silent: true }); } catch(_) {} display.success(`Removed ${ids.length} images`); }
          else { display.success('No images found'); }
        } catch (e) { display.warning(`Image cleanup warning: ${e.message}`); }

        // Remove ALL custom networks (Docker won't remove default networks)
        display.info('[3/5] Removing ALL custom networks...');
        try {
          const net = await execDocker(['network','ls','--filter','type=custom','-q'], { silent: true });
          const nets = (net.stdout || '').trim().split('\n').filter(Boolean);
          if (nets.length) {
            try { await execDocker(['network','rm', ...nets], { silent: true }); } catch(_) {}
            display.success(`Removed ${nets.length} custom networks`);
          } else {
            display.success('No custom networks found');
          }
        } catch (e) { display.warning(`Network cleanup warning: ${e.message}`); }

        // Remove ALL volumes
        display.info('[4/5] Removing ALL volumes...');
        try {
          const vr = await execDocker(['volume','ls','-q'], { silent: true });
          const vols = (vr.stdout || '').trim().split('\n').filter(Boolean);
          if (vols.length) { try { await execDocker(['volume','rm','-f', ...vols], { silent: true }); } catch(_) {} display.success(`Removed ${vols.length} volumes`); }
          else { display.success('No volumes found'); }
        } catch (e) { display.warning(`Volume cleanup warning: ${e.message}`); }

        // Optional prune caches
        display.info('[5/5] Pruning build cache and dangling resources...');
        try { await execDocker(['system','prune','-af','--volumes'], { silent: true }); display.success('System pruned'); } catch(_) {}

        console.log();
        display.success('Docker reset completed!');
      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
