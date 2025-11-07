const chalk = require('chalk');
const path = require('path');
const { spawn } = require('child_process');
const docker = require('../lib/docker');
const display = require('../lib/display');

async function exec(cmd, args = [], opts = {}) {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { stdio: opts.silent ? ['ignore','pipe','pipe'] : 'inherit', shell: false });
    let out = '';
    let err = '';
    if (opts.silent) {
      child.stdout.on('data', (d) => out += d.toString());
      child.stderr.on('data', (d) => err += d.toString());
    }
    child.on('exit', (code) => resolve({ code, stdout: out, stderr: err }));
    child.on('error', (e) => resolve({ code: 1, stdout: '', stderr: e.message }));
  });
}

module.exports = (program) => {
  program
    .command('fix')
    .description('Attempt to repair common Docker/env issues for TaskSandbox')
    .option('--pull', 'Pull TaskSandbox Docker images')
    .option('--prune', 'Prune dangling images/cache after cleanup')
    .option('--sandboxes', 'Also force-remove all tsbx sandbox containers')
    .option('--link', 'Run ./scripts/link.sh if present (dev)')
    .addHelpText('after', '\n' +
      'This command replaces ad-hoc setup.sh steps by applying safe host-side fixes.\n' +
      '\nActions performed:\n' +
      '  • Validate Docker availability\n' +
      '  • Ensure network/volumes exist\n' +
      '  • Remove exited tsbx_* containers (optionally all sandboxes)\n' +
      '  • Optional: pull images, prune caches\n' +
      '  • Quick GPU accessibility test\n' +
      '\nExamples:\n' +
      '  $ tsbx fix\n' +
      '  $ tsbx fix --pull\n' +
      '  $ tsbx fix --prune --sandboxes\n' +
      '  $ tsbx fix --link\n')
    .action(async (options) => {
      try {
        display.showCommandBox(`${display.icons.reset} TaskSandbox Fix`, { operation: 'Repair Docker/env for TaskSandbox' });

        // 1) Docker availability
        display.info('[1/6] Checking Docker availability...');
        const dockerOk = await docker.checkDocker();
        if (!dockerOk) {
          display.error('Docker is not available. Install/start Docker and retry.');
          process.exit(1);
        }
        display.success('Docker is available');

        // 2) Ensure network
        display.info('[2/6] Ensuring network tsbx_network exists...');
        try {
          await docker.execDocker(['network', 'inspect', 'tsbx_network'], { silent: true });
          display.success('Network exists');
        } catch (_) {
          await docker.execDocker(['network', 'create', 'tsbx_network']);
          display.success('Network created');
        }

        // 3) Ensure volumes
        display.info('[3/6] Ensuring required volumes exist...');
        const volumes = ['mysql_data','tsbx_snapshots_data'];
        for (const v of volumes) {
          try {
            await docker.execDocker(['volume','inspect', v], { silent: true });
          } catch (_) {
            await docker.execDocker(['volume','create', v]);
          }
        }
        display.success('Volumes ready');

        // 4) Remove exited tsbx containers
        display.info('[4/6] Removing exited tsbx_* containers...');
        try {
          const list = await docker.execDocker(['ps','-a','-q','--filter','name=tsbx_','--filter','status=exited'], { silent: true });
          const ids = (list.stdout || '').trim().split('\n').filter(Boolean);
          if (ids.length) {
            try { await docker.execDocker(['rm','-f', ...ids], { silent: true }); } catch (_) {}
            display.success(`Removed ${ids.length} exited containers`);
          } else {
            display.success('No exited containers to remove');
          }
        } catch (e) {
          display.warning('Could not list/remove exited containers: ' + e.message);
        }

        // Optional: remove ALL sandbox containers
        if (options.sandboxes) {
          display.info('Removing ALL tsbx sandbox containers (force)...');
          try {
            const r = await docker.execDocker(['ps','-a','-q','--filter','name=tsbx_sandbox_'], { silent: true });
            const ids = (r.stdout || '').trim().split('\n').filter(Boolean);
            if (ids.length) { try { await docker.execDocker(['rm','-f', ...ids], { silent: true }); } catch (_) {} display.success(`Removed ${ids.length} sandbox containers`); }
            else { display.success('No sandbox containers found'); }
          } catch (e) { display.warning('Sandbox cleanup warning: ' + e.message); }
        }

        // 5) Optional: pull images
        if (options.pull) {
          display.info('[5/6] Pulling TaskSandbox images (latest)...');
          try { await docker.pull('latest'); display.success('Images pulled'); } catch (e) { display.warning('Image pull warning: ' + e.message); }
        } else {
          display.info('[5/6] Skipping image pull (use --pull to enable)');
        }

        // Optional: prune
        if (options.prune) {
          display.info('Pruning dangling images/cache...');
          try { await docker.execDocker(['system','prune','-af','--volumes'], { silent: true }); display.success('Docker system pruned'); } catch (_) {}
        }

        // Dev convenience: link CLI
        if (options.link) {
          display.info('Linking CLI via ./scripts/link.sh (if present)...');
          const repoRoot = path.resolve(__dirname, '..', '..');
          const scriptPath = path.join(repoRoot, 'scripts', 'link.sh');
          const res = await exec('bash', ['-lc', `test -x ${scriptPath} && ${scriptPath} || true`], { silent: true });
          if (res.code === 0) display.success('Link script executed'); else display.warning('Link script not found or failed');
        }

        // 6) Quick GPU container test
        display.info('[6/6] Testing GPU accessibility (if available)...');
        const smi = await exec('bash', ['-lc', 'command -v nvidia-smi >/dev/null 2>&1']);
        if (smi.code === 0) {
          const test = await exec('docker', ['run','--rm','--gpus','all','nvidia/cuda:12.4.1-base-ubuntu22.04','nvidia-smi'], { silent: true });
          if (test.code === 0) display.success('GPU accessible inside containers'); else display.warning('GPU not accessible to containers');
        } else {
          display.info('nvidia-smi not present; skipping GPU test');
        }

        console.log();
        display.success('Fix completed. You can now try:');
        console.log('  • Start services: tsbx start');
        console.log('  • Check status:  docker ps --filter name=tsbx_');
      } catch (error) {
        console.error(chalk.red('Error:'), error.message);
        process.exit(1);
      }
    });
};
