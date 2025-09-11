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
    .description('Attempt to repair common Docker/env issues for Raworc')
    .option('--pull', 'Pull Raworc Docker images')
    .option('--prune', 'Prune dangling images/cache after cleanup')
    .option('--agents', 'Also force-remove all raworc agent containers')
    .option('--link', 'Run ./scripts/link.sh if present (dev)')
    .addHelpText('after', '\n' +
      'This command replaces ad-hoc setup.sh steps by applying safe host-side fixes.\n' +
      '\nActions performed:\n' +
      '  • Validate Docker availability\n' +
      '  • Ensure network/volumes exist\n' +
      '  • Remove exited raworc_* containers (optionally all agents)\n' +
      '  • Optional: pull images, prune caches\n' +
      '  • Quick GPU accessibility test\n' +
      '\nExamples:\n' +
      '  $ raworc fix\n' +
      '  $ raworc fix --pull\n' +
      '  $ raworc fix --prune --agents\n' +
      '  $ raworc fix --link\n')
    .action(async (options) => {
      try {
        display.showCommandBox(`${display.icons.reset} Raworc Fix`, { operation: 'Repair Docker/env for Raworc' });

        // 1) Docker availability
        display.info('[1/6] Checking Docker availability...');
        const dockerOk = await docker.checkDocker();
        if (!dockerOk) {
          display.error('Docker is not available. Install/start Docker and retry.');
          process.exit(1);
        }
        display.success('Docker is available');

        // 2) Ensure network
        display.info('[2/6] Ensuring network raworc_network exists...');
        try {
          await docker.execDocker(['network', 'inspect', 'raworc_network'], { silent: true });
          display.success('Network exists');
        } catch (_) {
          await docker.execDocker(['network', 'create', 'raworc_network']);
          display.success('Network created');
        }

        // 3) Ensure volumes
        display.info('[3/6] Ensuring required volumes exist...');
        const volumes = ['mysql_data','raworc_content_data','ollama_data','raworc_api_data','raworc_operator_data','raworc_controller_data'];
        for (const v of volumes) {
          try {
            await docker.execDocker(['volume','inspect', v], { silent: true });
          } catch (_) {
            await docker.execDocker(['volume','create', v]);
          }
        }
        display.success('Volumes ready');

        // 4) Remove exited raworc containers
        display.info('[4/6] Removing exited raworc_* containers...');
        try {
          const list = await docker.execDocker(['ps','-a','-q','--filter','name=raworc_','--filter','status=exited'], { silent: true });
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

        // Optional: remove ALL agent containers
        if (options.agents) {
          display.info('Removing ALL raworc agent containers (force)...');
          try {
            const r = await docker.execDocker(['ps','-a','-q','--filter','name=raworc_agent_'], { silent: true });
            const ids = (r.stdout || '').trim().split('\n').filter(Boolean);
            if (ids.length) { try { await docker.execDocker(['rm','-f', ...ids], { silent: true }); } catch (_) {} display.success(`Removed ${ids.length} agent containers`); }
            else { display.success('No agent containers found'); }
          } catch (e) { display.warning('Agent cleanup warning: ' + e.message); }
        }

        // 5) Optional: pull images
        if (options.pull) {
          display.info('[5/6] Pulling Raworc images (latest)...');
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
        console.log('  • Start services: raworc start');
        console.log('  • Check status:  docker ps --filter name=raworc_');
      } catch (error) {
        console.error(chalk.red('Error:'), error.message);
        process.exit(1);
      }
    });
};

