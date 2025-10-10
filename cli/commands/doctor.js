const { spawn } = require('child_process');
const chalk = require('chalk');

function exec(cmd, args = [], opts = {}) {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { stdio: ['ignore','pipe','pipe'], shell: false });
    let out = '';
    let err = '';
    child.stdout.on('data', (d) => out += d.toString());
    child.stderr.on('data', (d) => err += d.toString());
    child.on('exit', (code) => resolve({ code, stdout: out, stderr: err }));
    child.on('error', (e) => resolve({ code: 1, stdout: '', stderr: e.message }));
  });
}

function ok(msg) { console.log(`${chalk.green('✓')} ${msg}`); }
function bad(msg) { console.log(`${chalk.red('✗')} ${msg}`); }
function warn(msg) { console.log(`${chalk.yellow('!')} ${msg}`); }
function info(msg) { console.log(`${chalk.blue('i')} ${msg}`); }

module.exports = (program) => {
  program
    .command('doctor')
    .description('Run environment diagnostics and show GPU/Docker status')
    .action(async () => {
      try {
        info('Ractor Doctor — checking host readiness and GPU access');

        // OS detection
        const osres = await exec('bash', ['-lc', 'source /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" || uname -a']);
        if (osres.code === 0 && osres.stdout.trim()) {
          ok(`OS detected: ${osres.stdout.trim()}`);
        } else {
          warn('Unable to detect OS');
        }

        // GPU presence
        const lspci = await exec('bash', ['-lc', 'lspci | grep -i nvidia']);
        if (lspci.code === 0 && lspci.stdout.trim()) {
          ok('NVIDIA GPU: present');
        } else {
          warn('NVIDIA GPU: not detected (CPU mode only)');
        }

        // nvidia-smi
        const hasSmi = await exec('bash', ['-lc', 'command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi >/dev/null 2>&1']);
        if (hasSmi.code === 0) {
          ok('nvidia-smi: working');
        } else {
          warn('nvidia-smi: not found or not working');
        }

        // Docker
        const dv = await exec('docker', ['--version']);
        if (dv.code === 0) {
          ok(`Docker: installed (${dv.stdout.trim()})`);
        } else {
          bad('Docker: not installed or not in PATH');
        }

        // Docker runtime
        const di = await exec('docker', ['info']);
        if (di.code === 0) {
          const hasNvidia = /nvidia/i.test(di.stdout);
          if (hasNvidia) ok('Docker runtime: nvidia available'); else warn('Docker runtime: nvidia NOT available');
        } else {
          warn('Docker info not accessible (daemon down?)');
        }

        // CUDA container test
        info('Testing CUDA container access (may take a moment)...');
        const cuda = await exec('docker', ['run','--rm','--gpus','all','nvidia/cuda:12.4.1-base-ubuntu22.04','nvidia-smi']);
        if (cuda.code === 0) ok('CUDA container test: success (GPU accessible)'); else warn('CUDA container test: failed (GPU not accessible to containers)');

        console.log();
        ok('Diagnostics completed.');
      } catch (err) {
        console.error(chalk.red('Error running doctor:'), err.message);
        process.exit(1);
      }
    });
};
