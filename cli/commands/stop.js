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
    .description('Stop and remove specified Raworc component container(s) (no implicit all)')
    .argument('<components...>', 'Components to stop (mysql, ollama, server, operator, content, controller, gateway)')
    .option('-c, --cleanup', 'Clean up agent containers after stopping')
    .option('-v, --volumes', 'Remove named volumes after stopping')
    .option('-n, --network', 'Remove Docker network after stopping')
    .action(async (components, options) => {
      try {
        if (!components || components.length === 0) {
          console.log(chalk.red('[ERROR] ') + 'Please specify one or more components to stop');
          process.exit(1);
        }
        console.log(chalk.blue('[INFO] ') + 'Stopping Raworc services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Cleanup agent containers: ${!!options.cleanup}`);
        console.log(chalk.blue('[INFO] ') + `Remove volumes: ${!!options.volumes}`);
        console.log(chalk.blue('[INFO] ') + `Remove network: ${!!options.network}`);
        console.log(chalk.blue('[INFO] ') + `Components: ${components.join(', ')}`);

        console.log();

        const map = { mysql: 'raworc_mysql', server: 'raworc_server', controller: 'raworc_controller', ollama: 'raworc_ollama', operator: 'raworc_operator', content: 'raworc_content', gateway: 'raworc_gateway' };
        const order = ['gateway','controller','operator','content','server','ollama','mysql'];
        const toStop = order.filter((c) => components.includes(c));

        for (const comp of toStop) {
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

        if (options.cleanup) {
          console.log(chalk.blue('[INFO] ') + 'Cleaning up agent containers...');
          try {
            const res = await docker(['ps','-a','-q','--filter','name=raworc_agent_'], { silent: true });
            const ids = res.stdout.trim().split('\n').filter(Boolean);
            if (ids.length) {
              await docker(['stop', ...ids]);
              await docker(['rm', ...ids]);
              console.log(chalk.green('[SUCCESS] ') + `Cleaned up ${ids.length} agent containers`);
            } else {
              console.log(chalk.green('[SUCCESS] ') + 'No agent containers found');
            }
          } catch (e) {
            console.log(chalk.yellow('[WARNING] ') + 'Some agent containers could not be cleaned up');
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
        if (!options.remove) {
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Services stopped but containers preserved.');
          console.log(chalk.blue('[INFO] ') + 'To start again: raworc start');
          console.log(chalk.blue('[INFO] ') + 'To remove containers: raworc stop --remove');
        }
        if (!options.volumes && options.remove) {
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Containers removed but volumes preserved.');
          console.log(chalk.blue('[INFO] ') + 'To remove volumes: raworc stop --volumes');
        }
      } catch (error) {
        console.error(chalk.red('[ERROR] ') + (error && error.message ? error.message : String(error)));
        process.exit(1);
      }
    });
};
