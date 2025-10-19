const chalk = require('chalk');
const { spawn } = require('child_process');

function execCmd(cmd, args = [], opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: opts.silent ? 'pipe' : 'inherit', shell: false });
    let stdout = '';
    let stderr = '';
    if (opts.silent) {
      child.stdout.on('data', (d) => (stdout += d.toString()))
      child.stderr.on('data', (d) => (stderr += d.toString()))
    }
    child.on('exit', (code) => {
      if (code === 0) return resolve({ code, stdout, stderr });
      reject(new Error(stderr || `Command failed: ${cmd} ${args.join(' ')}`));
    });
    child.on('error', (err) => reject(err));
  });
}

async function docker(args, opts = {}) {
  return execCmd('docker', args, opts);
}

module.exports = (program) => {
  program
    .command('stop')
    .description('Stop and remove Ractor component containers (defaults to all if none specified)')
    .argument('[components...]', 'Components to stop. Allowed: api, controller, operator, content, gateway, sessions (all session containers). If omitted, stops core Ractor components; stop app components explicitly.')
    .addHelpText('after', '\n' +
      'Notes:\n' +
      '  • Stops and removes only Ractor component containers.\n' +
      '  • Does not remove images, volumes, or networks.\n' +
      '  • Use component "sessions" to stop/remove all session containers.\n' +
      '\nExamples:\n' +
      '  $ ractor stop                     # stop all Ractor components\n' +
      '  $ ractor stop api controller      # stop specific components\n' +
      '  $ ractor stop operator content    # stop UI components\n' +
      '  $ ractor stop sessions              # stop all session containers\n')
    .action(async (components, _opts, cmd) => {
      try {
        // Default to stopping all Ractor components when none specified
        if (!components || components.length === 0) {
          components = ['gateway','controller','operator','content','api'];
        }
        // Validate component names (only Ractor components)
        const allowed = new Set(['api','controller','operator','content','gateway','sessions']);
        const invalid = components.filter(c => !allowed.has(c));
        if (invalid.length) {
          console.log(chalk.red('[ERROR] ') + `Invalid component(s): ${invalid.join(', ')}. Allowed: api, controller, operator, content, gateway, sessions`);
          cmd.help({ error: true });
        }

        console.log(chalk.blue('[INFO] ') + 'Stopping Ractor services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Components: ${components.join(', ')}`);

        console.log();

        const map = { api: 'ractor_api', controller: 'ractor_controller', operator: 'ractor_operator', content: 'ractor_content', gateway: 'ractor_gateway' };
        const includeSessions = components.includes('sessions');
        const order = ['gateway','controller','operator','content','api'];
        const toStop = components.filter(c => c !== 'sessions');
        const ordered = order.filter((c) => toStop.includes(c));

        // Helper to list all containers matching a base name, including suffixed variants
        async function listMatchingContainers(base) {
          try {
            const res = await docker(['ps','-a','--format','{{.Names}}','--filter',`name=${base}`], { silent: true });
            const names = (res.stdout || '').trim().split('\n').filter(Boolean);
            return names.filter(n => n === base || n.startsWith(base + '_') || n.startsWith(base + '-'));
          } catch (_) { return []; }
        }

        for (const comp of ordered) {
          const baseName = map[comp];
          const matches = await listMatchingContainers(baseName);
          if (!matches.length) {
            console.log(chalk.green('[SUCCESS] ') + `${comp}: no containers found (base ${baseName})`);
            console.log();
            continue;
          }

          console.log(chalk.blue('[INFO] ') + `Stopping ${comp} containers: ${matches.join(', ')}`);
          for (const name of matches) {
            try {
              const running = await docker(['ps','-q','--filter',`name=^${name}$`], { silent: true });
              if (running.stdout.trim()) {
                await docker(['stop', name]);
                console.log(chalk.green('[SUCCESS] ') + `Stopped ${name}`);
              }
            } catch (e) {
              console.log(chalk.yellow('[WARNING] ') + `Failed to stop ${name}: ${e.message}`);
            }
          }

          console.log(chalk.blue('[INFO] ') + `Removing ${comp} containers...`);
          try {
            await docker(['rm','-f', ...matches]);
            console.log(chalk.green('[SUCCESS] ') + `Removed ${matches.length} container(s) for ${comp}`);
          } catch (e) {
            console.log(chalk.yellow('[WARNING] ') + `Failed to remove some ${comp} containers: ${e.message}`);
          }
          console.log();
        }

        if (includeSessions) {
          console.log(chalk.blue('[INFO] ') + 'Stopping session containers...');
          try {
            const res = await docker(['ps','-a','--format','{{.Names}}','--filter','name=ractor_session_'], { silent: true });
            const names = res.stdout.trim().split('\n').filter(Boolean);
            if (names.length) {
              try { await docker(['stop', ...names]); } catch (_) {}
              await docker(['rm','-f', ...names]);
              console.log(chalk.green('[SUCCESS] ') + `Stopped and removed ${names.length} session containers`);
            } else {
              console.log(chalk.green('[SUCCESS] ') + 'No session containers found');
            }
          } catch (e) {
            console.log(chalk.yellow('[WARNING] ') + 'Some session containers could not be removed');
          }
          console.log();
        }

        // No volumes or network removal here by design

        console.log(chalk.blue('[INFO] ') + 'Checking remaining services...');
        console.log();
        let status = '';
        try { const res = await docker(['ps','--filter','name=ractor_','--format','table {{.Names}}\t{{.Status}}\t{{.Ports}}'], { silent: true }); status = res.stdout; } catch(_) {}
        if (status && status.trim() && status.trim() !== 'NAMES\tSTATUS\tPORTS') {
          console.log(status);
          console.log();
          console.log(chalk.yellow('[WARNING] ') + 'Some Ractor containers are still running');
        } else {
          console.log(chalk.green('[SUCCESS] ') + 'No Ractor containers are running');
        }

        console.log();
        console.log(chalk.green('[SUCCESS] ') + '🛑 Stop completed!');
      } catch (error) {
        console.error(chalk.red('[ERROR] ') + (error && error.message ? error.message : String(error)));
        process.exit(1);
      }
    });
};
