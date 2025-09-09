const chalk = require('chalk');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean Raworc Docker resources by type(s): containers images volumes networks')
    .argument('<types...>', 'Types to clean: containers | images | volumes | networks (one or more)')
    .action(async (types) => {
      try {
        const valid = new Set(['containers','images','volumes','networks']);
        const list = Array.isArray(types) ? types : [types];
        const invalid = list.filter(t => !valid.has(t));
        if (invalid.length) {
          display.error(`Invalid type(s): ${invalid.join(', ')}. Use only: containers, images, volumes, networks`);
          process.exit(1);
        }
        // Show command box
        display.showCommandBox(`${display.icons.clean} Clean Raworc`, { operation: `Remove: ${list.join(', ')}` });

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available');
          process.exit(1);
        }

        if (list.includes('containers')) {
          display.info('Stopping and removing Raworc containers...');
          const res = await docker.execDocker(['ps', '-a', '--format', '{{.Names}}'], { silent: true });
          const names = (res.stdout || '').trim().split('\n').filter(Boolean).filter(n => /^raworc_/.test(n) || /^raworc_agent_/.test(n));
          if (names.length) {
            try { await docker.execDocker(['stop', ...names], { silent: true }); } catch(_) {}
            try { await docker.execDocker(['rm', '-f', ...names], { silent: true }); } catch(_) {}
            display.success(`Removed ${names.length} Raworc containers`);
          } else {
            display.success('No Raworc containers found');
          }
        }

        if (list.includes('images')) {
          display.info('Removing Raworc images...');
          const res = await docker.execDocker(['images', '--format', '{{.Repository}}:{{.Tag}} {{.ID}}'], { silent: true });
          const lines = (res.stdout || '').trim().split('\n').filter(Boolean);
          const imgs = lines.map(l => ({ ref: l.split(' ')[0], id: l.split(' ')[1] }))
            .filter(o => /^raworc\//.test(o.ref) || /^raworc_/.test(o.ref));
          const ids = imgs.map(o => o.id);
          if (ids.length) {
            try { await docker.execDocker(['rmi', '-f', ...ids], { silent: true }); } catch(_) {}
            display.success(`Removed ${ids.length} Raworc images`);
          } else {
            display.success('No Raworc images found');
          }
        }

        if (list.includes('volumes')) {
          display.info('Removing Raworc volumes...');
          const res = await docker.execDocker(['volume', 'ls', '--format', '{{.Name}}'], { silent: true });
          const vols = (res.stdout || '').trim().split('\n').filter(Boolean).filter(v => /^raworc_/.test(v));
          if (vols.length) {
            try { await docker.execDocker(['volume', 'rm', '-f', ...vols], { silent: true }); } catch(_) {}
            display.success(`Removed ${vols.length} Raworc volumes`);
          } else {
            display.success('No Raworc volumes found');
          }
        }

        if (list.includes('networks')) {
          display.info('Removing Raworc networks...');
          try { await docker.execDocker(['network', 'rm', 'raworc_network'], { silent: true }); display.success('Removed network raworc_network'); }
          catch(_) { display.success('Network raworc_network not present'); }
        }

        console.log();
        display.success('Clean completed');
      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
