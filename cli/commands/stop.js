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
    .description('Stop and remove containers for specific components (no implicit all)')
    .argument('<components...>', 'Components to stop. Allowed: mysql, ollama, server, controller, operator, content, gateway, agents (all agent containers)')
    .addHelpText('after', '\n' +
      'Notes:\n' +
      '  • Stops and removes the specified component containers only.\n' +
      '  • Does not remove images, volumes, or networks.\n' +
      '  • Use component "agents" to stop/remove all agent containers.\n' +
      '\nExamples:\n' +
      '  $ raworc stop server controller\n' +
      '  $ raworc stop agents\n' +
      '  $ raworc stop mysql ollama\n')
    .action(async (components) => {
      try {
        if (!components || components.length === 0) {
          console.log(chalk.red('[ERROR] ') + 'Please specify components to stop (e.g., server controller agents)');
          process.exit(1);
        }
        console.log(chalk.blue('[INFO] ') + 'Stopping Raworc services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Components: ${components.join(', ')}`);

        console.log();

        const map = { mysql: 'mysql', server: 'raworc_server', controller: 'raworc_controller', ollama: 'ollama', operator: 'raworc_operator', content: 'raworc_content', gateway: 'raworc_gateway' };
        const includeAgents = components.includes('agents');
        const order = ['gateway','controller','operator','content','server','ollama','mysql'];
        const toStop = components.filter(c => c !== 'agents');
        const ordered = order.filter((c) => toStop.includes(c));

        for (const comp of ordered) {
          const name = map[comp];
          console.log(chalk.blue('[INFO] ') + `Stopping ${comp} (${name})...`);
          try {
            const running = await docker(['ps','-q','--filter',`name=${name}`], { silent: true });
            if (running.stdout.trim()) {
              await docker(['stop', name]);
              console.log(chalk.green('[SUCCESS] ') + `Stopped ${comp}`);
            } else {
              console.log(chalk.green('[SUCCESS] ') + `${comp} is not running`);
            }
          } catch (e) {
            console.log(chalk.red('[ERROR] ') + `Failed to stop ${comp}: ${e.message}`);
          }
          // Always remove container after stopping
          console.log(chalk.blue('[INFO] ') + `Removing ${comp} container...`);
          try {
            const exists = await docker(['ps','-aq','--filter',`name=${name}`], { silent: true });
            if (exists.stdout.trim()) {
              await docker(['rm', '-f', name]);
              console.log(chalk.green('[SUCCESS] ') + `Removed ${comp} container`);
            } else {
              console.log(chalk.green('[SUCCESS] ') + `${comp} container already removed`);
            }
          } catch (e) {
            console.log(chalk.yellow('[WARNING] ') + `Failed to remove ${comp} container`);
          }
          console.log();
        }

        if (includeAgents) {
          console.log(chalk.blue('[INFO] ') + 'Stopping agent containers...');
          try {
            const res = await docker(['ps','-a','--format','{{.Names}}','--filter','name=raworc_agent_'], { silent: true });
            const names = res.stdout.trim().split('\n').filter(Boolean);
            if (names.length) {
              try { await docker(['stop', ...names]); } catch (_) {}
              await docker(['rm','-f', ...names]);
              console.log(chalk.green('[SUCCESS] ') + `Stopped and removed ${names.length} agent containers`);
            } else {
              console.log(chalk.green('[SUCCESS] ') + 'No agent containers found');
            }
          } catch (e) {
            console.log(chalk.yellow('[WARNING] ') + 'Some agent containers could not be removed');
          }
          console.log();
        }

        if (options.volumes) {
          console.log(chalk.blue('[INFO] ') + 'Removing volumes...');
          for (const v of ['raworc_mysql_data','raworc_content_data','raworc_ollama_data','raworc_logs']) {
            try {
              await docker(['volume','inspect', v], { silent: true });
              await docker(['volume','rm', v]);
              console.log(chalk.green('[SUCCESS] ') + `Removed volume ${v}`);
            } catch (_) {
              console.log(chalk.green('[SUCCESS] ') + `Volume ${v} already removed`);
            }
          }
          console.log();
        }

        if (options.network) {
          console.log(chalk.blue('[INFO] ') + 'Removing Docker network...');
          try { await docker(['network','rm','raworc_network']); console.log(chalk.green('[SUCCESS] ') + 'Removed raworc_network'); } catch(_) { console.log(chalk.green('[SUCCESS] ') + 'Network raworc_network already removed'); }
          console.log();
        }

        console.log(chalk.blue('[INFO] ') + 'Checking remaining services...');
        console.log();
        let status = '';
        try { const res = await docker(['ps','--filter','name=raworc_','--format','table {{.Names}}\t{{.Status}}\t{{.Ports}}'], { silent: true }); status = res.stdout; } catch(_) {}
        if (status && status.trim() && status.trim() !== 'NAMES\tSTATUS\tPORTS') {
          console.log(status);
          console.log();
          console.log(chalk.yellow('[WARNING] ') + 'Some Raworc containers are still running');
        } else {
          console.log(chalk.green('[SUCCESS] ') + 'No Raworc containers are running');
        }

        console.log();
        console.log(chalk.green('[SUCCESS] ') + '🛑 Stop completed!');
      } catch (error) {
        console.error(chalk.red('[ERROR] ') + (error && error.message ? error.message : String(error)));
        process.exit(1);
      }
    });
};
