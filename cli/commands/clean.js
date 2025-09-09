const chalk = require('chalk');
const docker = require('../lib/docker');
const display = require('../lib/display');

module.exports = (program) => {
  program
    .command('clean')
    .description('Clean Raworc resources selectively')
    .option('--containers', 'Remove Raworc containers (raworc_* and raworc_agent_*)')
    .option('--images', 'Remove Raworc images (raworc/* and local raworc_*)')
    .option('--volumes', 'Remove Raworc volumes (raworc_* and raworc_agent_data_*)')
    .option('--networks', 'Remove Raworc networks (raworc_network)')
    .action(async (options) => {
      try {
        const anySelected = options.containers || options.images || options.volumes || options.networks;
        if (!anySelected) {
          display.error('Please specify at least one of --containers --images --volumes --networks');
          process.exit(1);
        }

        // Show command box with clean info
        display.showCommandBox(`${display.icons.clean} Clean Raworc Resources`, {
          operation: [
            options.containers ? 'containers' : null,
            options.images ? 'images' : null,
            options.volumes ? 'volumes' : null,
            options.networks ? 'networks' : null,
          ].filter(Boolean).join(', ')
        });

        // Check Docker availability
        const dockerAvailable = await docker.checkDocker();
        if (!dockerAvailable) {
          display.error('Docker is not available');
          process.exit(1);
        }

        try {
          // Containers
          if (options.containers) {
            display.info('Removing Raworc containers...');
            const res = await docker.execDocker(['ps', '-a', '--format', '{{.Names}}']);
            const names = (res.stdout || '').trim().split('\n').filter(Boolean);
            const raworcNames = names.filter(n => /^raworc_/.test(n) || /^raworc_agent_/.test(n));
            if (raworcNames.length) {
              // Stop running ones first
              try { await docker.execDocker(['stop', ...raworcNames], { silent: true }); } catch (_) {}
              try { await docker.execDocker(['rm', '-f', ...raworcNames], { silent: true }); } catch (e) {
                display.warning('Some containers could not be removed');
              }
              display.success(`Removed ${raworcNames.length} containers`);
            } else {
              display.success('No Raworc containers found');
            }
            console.log();
          }

          // Images
          if (options.images) {
            display.info('Removing Raworc images...');
            const res = await docker.execDocker(['images', '--format', '{{.Repository}}:{{.Tag}} {{.ID}}'], { silent: true });
            const lines = (res.stdout || '').trim().split('\n').filter(Boolean);
            const imgs = lines.map(l => ({ ref: l.split(' ')[0], id: l.split(' ')[1] }))
              .filter(o => /^raworc\//.test(o.ref) || /^raworc_/.test(o.ref));
            const ids = imgs.map(o => o.id);
            if (ids.length) {
              try { await docker.execDocker(['rmi', '-f', ...ids], { silent: true }); } catch (e) {
                display.warning('Some images could not be removed');
              }
              display.success(`Removed ${ids.length} images`);
            } else {
              display.success('No Raworc images found');
            }
            console.log();
          }

          // Volumes
          if (options.volumes) {
            display.info('Removing Raworc volumes...');
            const res = await docker.execDocker(['volume', 'ls', '--format', '{{.Name}}'], { silent: true });
            const vols = (res.stdout || '').trim().split('\n').filter(Boolean)
              .filter(v => /^raworc_/.test(v));
            if (vols.length) {
              try { await docker.execDocker(['volume', 'rm', '-f', ...vols], { silent: true }); } catch (e) {
                display.warning('Some volumes could not be removed');
              }
              display.success(`Removed ${vols.length} volumes`);
            } else {
              display.success('No Raworc volumes found');
            }
            console.log();
          }

          // Networks
          if (options.networks) {
            display.info('Removing Raworc networks...');
            try {
              await docker.execDocker(['network', 'rm', 'raworc_network'], { silent: true });
              display.success('Removed network raworc_network');
            } catch (_) {
              display.success('Network raworc_network not present');
            }
            console.log();
          }

          display.success('Clean completed');
        } catch (error) {
          throw error;
        }

      } catch (error) {
        display.error('Error: ' + error.message);
        process.exit(1);
      }
    });
};
