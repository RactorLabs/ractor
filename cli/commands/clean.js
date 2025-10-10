const chalk = require('chalk');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean Ractor resources by type(s): containers, images, volumes, networks (scoped)')
    .argument('<types...>', 'One or more of: containers, images, volumes, networks')
    .addHelpText('after', '\n' +
      'Scope:\n' +
      '  • containers: names starting with ractor_\n' +
      '  • images: repositories registry.digitalocean.com/ractor/* or ractor/*, or names starting ractor_\n' +
      '  • volumes: names starting ractor_\n' +
      '  • networks: ractor_network only\n' +
      '\nExamples:\n' +
      '  $ ractor clean containers images\n' +
      '  $ ractor clean volumes networks\n')
    .action(async (types, _opts, cmd) => {
      try {
        const valid = new Set(['containers','images','volumes','networks']);
        const list = Array.isArray(types) ? types : [types];
        const invalid = list.filter(t => !valid.has(t));
        if (invalid.length) {
          display.error(`Invalid type(s): ${invalid.join(', ')}. Use only: containers, images, volumes, networks`);
          cmd.help({ error: true });
        }
        // Show command box
        display.showCommandBox(`${display.icons.clean} Clean Ractor`, { operation: `Remove: ${list.join(', ')}` });

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available');
          process.exit(1);
        }

        if (list.includes('containers')) {
          display.info('Stopping and removing Ractor containers...');
          const res = await docker.execDocker(['ps', '-a', '--format', '{{.Names}}'], { silent: true });
          const names = (res.stdout || '').trim().split('\n').filter(Boolean).filter(n => /^ractor_/.test(n));
          if (names.length) {
            try { await docker.execDocker(['stop', ...names], { silent: true }); } catch(_) {}
            try { await docker.execDocker(['rm', '-f', ...names], { silent: true }); } catch(_) {}
            display.success(`Removed ${names.length} Ractor containers`);
          } else {
            display.success('No Ractor containers found');
          }
        }

        if (list.includes('images')) {
          display.info('Removing Ractor images...');
          const res = await docker.execDocker(['images', '--format', '{{.Repository}}:{{.Tag}} {{.ID}}'], { silent: true });
          const lines = (res.stdout || '').trim().split('\n').filter(Boolean);
          const imgs = lines.map(l => ({ ref: l.split(' ')[0], id: l.split(' ')[1] }))
            .filter(o => /^(registry\.digitalocean\.com\/ractor\/|ractor\/)/.test(o.ref) || /^ractor_/.test(o.ref));
          const ids = imgs.map(o => o.id);
          if (ids.length) {
            try { await docker.execDocker(['rmi', '-f', ...ids], { silent: true }); } catch(_) {}
            display.success(`Removed ${ids.length} Ractor images`);
          } else {
            display.success('No Ractor images found');
          }
        }

        if (list.includes('volumes')) {
          display.info('Removing Ractor volumes...');
          const res = await docker.execDocker(['volume', 'ls', '--format', '{{.Name}}'], { silent: true });
          const vols = (res.stdout || '').trim().split('\n').filter(Boolean).filter(v => /^ractor_/.test(v));
          if (vols.length) {
            try { await docker.execDocker(['volume', 'rm', '-f', ...vols], { silent: true }); } catch(_) {}
            display.success(`Removed ${vols.length} Ractor volumes`);
          } else {
            display.success('No Ractor volumes found');
          }
        }

        if (list.includes('networks')) {
          display.info('Removing Ractor networks...');
          try { await docker.execDocker(['network', 'rm', 'ractor_network'], { silent: true }); display.success('Removed network ractor_network'); }
          catch(_) { display.success('Network ractor_network not present'); }
        }

        console.log();
        display.success('Clean completed');
      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
