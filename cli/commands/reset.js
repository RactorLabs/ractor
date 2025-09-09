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
    .description('Shortcut: clean Raworc containers, images, volumes, networks')
    .option('-y, --yes', 'Confirm without prompting (non-interactive)')
    .action(async (options) => {
      try {
        display.showCommandBox(`${display.icons.reset} Raworc Reset`, { operation: 'containers, images, volumes, networks' });

        if (!options.yes) {
          const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
          const answer = await new Promise((resolve) => rl.question(chalk.yellow('This will remove Raworc containers/images/volumes/networks. Continue? [y/N]: '), resolve));
          rl.close();
          if (!answer.match(/^[Yy]$/)) { display.info('Operation cancelled'); return; }
        }

        // Stop and remove containers
        display.info('[1/4] Removing Raworc containers...');
        try {
          const res = await execDocker(['ps','-a','--format','{{.Names}}'], { silent: true });
          const names = (res.stdout || '').trim().split('\n').filter(Boolean).filter(n => /^raworc_/.test(n) || /^raworc_agent_/.test(n));
          if (names.length) {
            try { await execDocker(['stop', ...names], { silent: true }); } catch(_) {}
            await execDocker(['rm','-f', ...names], { silent: true });
            display.success(`Removed ${names.length} Raworc containers`);
          } else {
            display.success('No Raworc containers found');
          }
        } catch (e) { display.warning(`Container cleanup warning: ${e.message}`); }

        // Remove images
        display.info('[2/4] Removing Raworc images...');
        try {
          const ir = await execDocker(['images','--format','{{.Repository}}:{{.Tag}} {{.ID}}'], { silent: true });
          const lines = (ir.stdout || '').trim().split('\n').filter(Boolean);
          const imgs = lines.map(l => ({ ref: l.split(' ')[0], id: l.split(' ')[1] })).filter(o => /^raworc\//.test(o.ref) || /^raworc_/.test(o.ref));
          const ids = imgs.map(o => o.id);
          if (ids.length) { try { await execDocker(['rmi','-f', ...ids], { silent: true }); } catch(_) {} display.success(`Removed ${ids.length} Raworc images`); }
          else { display.success('No Raworc images found'); }
        } catch (e) { display.warning(`Image cleanup warning: ${e.message}`); }

        // Remove network
        display.info('[3/4] Removing Raworc network...');
        try { await execDocker(['network','rm','raworc_network'], { silent: true }); display.success('Removed network raworc_network'); } catch(_) { display.success('Network raworc_network not present'); }

        // Remove volumes
        display.info('[4/4] Removing Raworc volumes...');
        try {
          const vr = await execDocker(['volume','ls','--format','{{.Name}}'], { silent: true });
          const vols = (vr.stdout || '').trim().split('\n').filter(Boolean).filter(v => /^raworc_/.test(v));
          if (vols.length) { try { await execDocker(['volume','rm','-f', ...vols], { silent: true }); } catch(_) {} display.success(`Removed ${vols.length} Raworc volumes`); }
          else { display.success('No Raworc volumes found'); }
        } catch (e) { display.warning(`Volume cleanup warning: ${e.message}`); }

        console.log();
        display.success('Raworc reset completed!');
      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
