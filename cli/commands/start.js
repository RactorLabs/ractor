const chalk = require('chalk');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const COMPONENT_ALIASES = {
  a: 'api',
  c: 'controller',
  o: 'operator',
};

function resolveComponentAliases(list = []) {
  return list.map((name) => {
    const lower = (name || '').toLowerCase();
    return COMPONENT_ALIASES[lower] || lower;
  });
}

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

async function portInUse(port) {
  try {
    const res = await execCmd('bash', ['-lc', `ss -ltn 2>/dev/null | awk '{print $4}' | grep -q ':${port}$'`], { silent: true });
    return res.code === 0; // grep matched
  } catch (_) {
    return false;
  }
}

function readProjectVersionOrLatest() {
  try {
    const cargoPath = path.join(process.cwd(), 'Cargo.toml');
    if (fs.existsSync(cargoPath)) {
      const content = fs.readFileSync(cargoPath, 'utf8');
      const m = content.match(/^version\s*=\s*"([^"]+)"/m);
      if (m) return m[1];
    }
  } catch (_) {}
  return 'latest';
}

async function ensureNetwork() {
  try {
    await docker(['network', 'inspect', 'tsbx_network'], { silent: true });
  } catch (_) {
    await docker(['network', 'create', 'tsbx_network']);
  }
}

async function ensureVolumes() {
  for (const v of ['mysql_data', 'tsbx_snapshots_data']) {
    try {
      await docker(['volume', 'inspect', v], { silent: true });
    } catch (_) {
      await docker(['volume', 'create', v]);
    }
  }
}

async function isDockerAvailable() {
  try { await docker(['--version'], { silent: true }); return true; } catch (_) { return false; }
}

async function waitForMysql() {
  process.stdout.write(chalk.blue('[INFO] ') + 'Waiting for MySQL to be ready...\n');
  for (let i = 0; i < 30; i++) {
    try {
      await docker(['exec', 'mysql', 'mysqladmin', 'ping', '-h', 'localhost', '-u', 'root', '-proot'], { silent: true });
      console.log(chalk.green('[SUCCESS] ') + 'MySQL is ready');
      return;
    } catch (_) {
      await new Promise(r => setTimeout(r, 2000));
    }
  }
  throw new Error('MySQL failed to become healthy');
}

module.exports = (program) => {
  program
    .command('start')
    .description('Start services: create if missing or start if stopped (never removes)')
    .argument('[components...]', 'Components to start. Default: core stack. Allowed: mysql, api, controller, operator, gateway (apps start only when listed). Shortcuts: a=api, c=controller, o=operator.', [])
    .option('-p, --pull', 'Pull base images (mysql) before starting')
    .option('-d, --detached', 'Run in detached mode', true)
    .option('-f, --foreground', 'Run MySQL in foreground mode')
    .option('--inference-model <model>', 'Inference model name', 'llama-3.2-3b-instruct-fast-tp2')
    .option('--inference-url <url>', 'Inference API base URL', 'https://api.positron.ai/v1')
    .option('--inference-api-key <key>', 'Inference API key (Bearer token)')
    // MySQL options
    .option('--mysql-port <port>', 'Host port for MySQL', '3307')
    .option('--mysql-root-password <pw>', 'MySQL root password', 'root')
    .option('--mysql-database <db>', 'MySQL database name', 'tsbx')
    .option('--mysql-user <user>', 'MySQL user', 'tsbx')
    .option('--mysql-password <pw>', 'MySQL user password', 'tsbx')
    // API options
    .option('--api-database-url <url>', 'API DATABASE_URL', 'mysql://tsbx:tsbx@mysql:3306/tsbx')
    .option('--api-jwt-secret <secret>', 'API JWT_SECRET')
    .option('--api-rust-log <level>', 'API RUST_LOG', 'info')
    .option('--api-tsbx-host <host>', 'API TSBX_HOST')
    .option('--api-tsbx-port <port>', 'API TSBX_PORT')
    .option('--api-api-port <port>', 'Host port for API (maps to 9000)', '9000')
    // Controller options
    .option('--controller-database-url <url>', 'Controller DATABASE_URL', 'mysql://tsbx:tsbx@mysql:3306/tsbx')
    .option('--controller-jwt-secret <secret>', 'Controller JWT_SECRET')
    .option('--controller-rust-log <level>', 'Controller RUST_LOG', 'info')
    .addHelpText('after', '\n' +
      'Notes:\n' +
      '  • Starts each component if stopped, or creates it if missing.\n' +
      '  • Does not stop or remove any containers.\n' +
      '  • MySQL container name is "mysql".\n' +
      '  • Component shortcuts: a=api, c=controller, o=operator.\n' +
      '\nExamples:\n' +
      '  $ tsbx start                                # Start full stack\n' +
      '  $ tsbx start api controller                 # Start API + controller\n' +
      '  $ tsbx start mysql                          # Ensure MySQL is up\n')
    .option('--controller-sandbox-image <image>', 'Controller SANDBOX_IMAGE')
    .option('--controller-sandbox-cpu-limit <n>', 'Controller SANDBOX_CPU_LIMIT', '0.5')
    .option('--controller-sandbox-memory-limit <bytes>', 'Controller SANDBOX_MEMORY_LIMIT', '536870912')
    .option('--controller-sandbox-disk-limit <bytes>', 'Controller SANDBOX_DISK_LIMIT', '1073741824')
    .action(async (inputComponents, options) => {
      try {
        let components = resolveComponentAliases(inputComponents);

        const detached = options.foreground ? false : (options.detached !== false);
        const tag = readProjectVersionOrLatest();

        // Resolve host branding and URL only here (script-level default allowed)
        const TSBX_HOST_NAME = process.env.TSBX_HOST_NAME || 'TaskSandbox';
        const TSBX_HOST_URL = (process.env.TSBX_HOST_URL || 'http://localhost').replace(/\/$/, '');

        function withPort(baseUrl, port) {
          try {
            const u = new URL(baseUrl);
            // If base already has a port, keep path-only joins; otherwise append port
            const host = u.hostname;
            const proto = u.protocol;
            return `${proto}//${host}:${String(port)}`;
          } catch (_) {
            return `${baseUrl}:${String(port)}`;
          }
        }

        async function imageExistsLocally(name) {
          try {
            const res = await docker(['images','-q', name], { silent: true });
            return !!res.stdout.trim();
          } catch (_) { return false; }
        }

        async function resolveTaskSandboxImage(component, localShortName, remoteRepo, tag) {
          const localName = `${localShortName}:${tag}`;
          if (await imageExistsLocally(localName)) {
            console.log(chalk.blue('[INFO] ') + `${component}: using local image ${localName}`);
            return localName;
          }
          const remoteTagged = `${remoteRepo}:${tag}`;
          const remoteLatest = `${remoteRepo}:latest`;
          console.log(chalk.blue('[INFO] ') + `${component}: local image not found (${localName}); pulling ${remoteTagged}...`);
          try {
            await docker(['pull', remoteTagged]);
            console.log(chalk.green('[SUCCESS] ') + `Pulled ${remoteTagged}`);
            return remoteTagged;
          } catch (e1) {
            console.log(chalk.yellow('[WARNING] ') + `Failed to pull ${remoteTagged}: ${e1.message}`);
            console.log(chalk.blue('[INFO] ') + `Trying ${remoteLatest}...`);
            try {
              await docker(['pull', remoteLatest]);
              console.log(chalk.green('[SUCCESS] ') + `Pulled ${remoteLatest}`);
              return remoteLatest;
            } catch (e2) {
              throw new Error(`Unable to find image for ${component}. Tried local ${localName}, remote ${remoteTagged} and ${remoteLatest}`);
            }
          }
        }

        // Note: resolve images lazily per requested component to avoid unnecessary pulls

        console.log(chalk.blue('[INFO] ') + 'Starting TaskSandbox services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Image tag: ${tag}`);
        console.log(chalk.blue('[INFO] ') + `Pull base images: ${!!options.pull}`);
        console.log(chalk.blue('[INFO] ') + `Detached mode: ${detached}`);

        if (!components || components.length === 0) {
          components = ['mysql', 'api', 'controller', 'operator', 'gateway'];
        }

        // Enforce startup order: mysql → api → controller
        const desiredOrder = ['mysql', 'api', 'controller', 'operator', 'gateway'];
        const unique = Array.from(new Set(components));
        const ordered = [];
        for (const name of desiredOrder) {
          if (unique.includes(name)) ordered.push(name);
        }
        // Append any unknown components at the end preserving input order
        for (const name of unique) {
          if (!desiredOrder.includes(name)) ordered.push(name);
        }
        components = ordered;
        console.log(chalk.blue('[INFO] ') + `Components: ${components.join(', ')}`);

        if (!(await isDockerAvailable())) {
          console.error(chalk.red('[ERROR] ') + 'Docker is not available. Please install Docker first.');
          process.exit(1);
        }

        console.log();

        // No build step in start; use ./scripts/build.sh in dev

        // Pull base images
        if (options.pull) {
          console.log(chalk.blue('[INFO] ') + 'Pulling base images...');
          try { await docker(['pull', 'mysql:8.0']); } catch (e) { console.log(chalk.yellow('[WARNING] ') + 'Failed to pull mysql:8.0; continuing...'); }
          // TaskSandbox images are resolved lazily when each component starts.
          console.log();
        }

        await ensureNetwork();
        await ensureVolumes();
        console.log();

        // Helpers: env precedence and parsing (must be defined before use)
        const getOptionSource = (name) => {
          try { return program.getOptionValueSource(name); } catch (_) { return undefined; }
        };
        const preferEnv = (optName, envName, defaultValue) => {
          const source = getOptionSource(optName);
          const optVal = options[optName];
          if (source === 'cli') return optVal; // explicit flag wins
          if (process.env[envName] !== undefined && process.env[envName] !== '') return process.env[envName];
          return optVal !== undefined ? optVal : defaultValue;
        };
        const envBool = (name, fallback) => {
          const v = process.env[name];
          if (v === undefined) return fallback;
          if (typeof v === 'string') {
            const s = v.trim().toLowerCase();
            if (['1','true','yes','y','on'].includes(s)) return true;
            if (['0','false','no','n','off'].includes(s)) return false;
          }
          return fallback;
        };
        const hostPort = (() => {
          const raw = process.env.TSBX_HOST_PORT;
          if (raw === undefined || raw.trim() === '') return '80';
          const trimmed = raw.trim();
          if (!/^\d+$/.test(trimmed)) {
            console.error(chalk.red('[ERROR] ') + 'TSBX_HOST_PORT must be a positive integer.');
            process.exit(1);
          }
          const numeric = parseInt(trimmed, 10);
          if (!Number.isFinite(numeric) || numeric <= 0 || numeric > 65535) {
            console.error(chalk.red('[ERROR] ') + 'TSBX_HOST_PORT must be between 1 and 65535.');
            process.exit(1);
          }
          return String(numeric);
        })();
        // Helpers for container state
        async function containerRunning(name) {
          try { const res = await docker(['ps','-q','--filter',`name=${name}`], { silent: true }); return !!res.stdout.trim(); } catch(_) { return false; }
        }
        async function containerExists(name) {
          try { const res = await docker(['ps','-aq','--filter',`name=${name}`], { silent: true }); return !!res.stdout.trim(); } catch(_) { return false; }
        }
        async function imageId(imageRef) {
          try {
            const res = await docker(['image','inspect', imageRef, '--format','{{.Id}}'], { silent: true });
            return res.stdout.trim();
          } catch (_) { return ''; }
        }
        async function containerImageId(containerName) {
          try {
            const res = await docker(['inspect', containerName, '--format','{{.Image}}'], { silent: true });
            return res.stdout.trim();
          } catch (_) { return ''; }
        }

        const INFERENCE_URL = (() => {
          const src = getOptionSource('inferenceUrl');
          if (src === 'cli') {
            return options.inferenceUrl;
          }
          const envUrl = process.env.TSBX_INFERENCE_URL;
          if (envUrl && envUrl.trim() !== '') {
            return envUrl;
          }
          return options.inferenceUrl || 'https://api.positron.ai/v1';
        })();
        const INFERENCE_API_KEY = options.inferenceApiKey || process.env.TSBX_INFERENCE_API_KEY || '6V-E5ROIlFIgSVgmL8hcluSAistpSEbi-UcbIHwHuoM';
        const INFERENCE_MODEL = (() => {
          const src = getOptionSource('inferenceModel');
          if (src === 'cli') {
            return options.inferenceModel;
          }
          const envModel = process.env.TSBX_INFERENCE_MODEL;
          if (envModel && envModel.trim() !== '') {
            return envModel;
          }
          return options.inferenceModel || 'llama-3.2-3b-instruct-fast-tp2';
        })();
        const normalizeTemplate = (raw) => {
          const value = (raw || '').trim().toLowerCase();
          if (!value) return 'openai';
          if (value === 'positron') return 'positron';
          if (value === 'openai') return 'openai';
          return value;
        };
        const INFERENCE_TEMPLATE = normalizeTemplate(options.inferenceTemplate || process.env.TSBX_INFERENCE_TEMPLATE || 'openai');

        for (const comp of components) {
          switch (comp) {
            case 'mysql': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring MySQL database is running...');
              if (await containerRunning('mysql')) { console.log(chalk.green('[SUCCESS] ') + 'MySQL already running'); console.log(); break; }
              if (await containerExists('mysql')) {
                await docker(['start','mysql']);
                console.log(chalk.green('[SUCCESS] ') + 'MySQL started');
                console.log();
                break;
              }
              const args = ['run'];
              if (detached) args.push('-d');
              args.push(
                '--name','mysql',
                '--network','tsbx_network',
                '-p', `${String(options.mysqlPort || '3307')}:3306`,
                '-v','mysql_data:/var/lib/mysql',
                '-e',`MYSQL_ROOT_PASSWORD=${options.mysqlRootPassword || 'root'}`,
                '-e',`MYSQL_DATABASE=${options.mysqlDatabase || 'tsbx'}`,
                '-e',`MYSQL_USER=${options.mysqlUser || 'tsbx'}`,
                '-e',`MYSQL_PASSWORD=${options.mysqlPassword || 'tsbx'}`,
                '--health-cmd','mysqladmin ping -h localhost -u root -proot',
                '--health-interval','10s',
                '--health-timeout','5s',
                '--health-retries','5',
                'mysql:8.0',
                // Persist logs into data volume
                '--log-error=/var/lib/mysql/mysql-error.log',
                '--slow_query_log=ON',
                '--long_query_time=2',
                '--slow_query_log_file=/var/lib/mysql/mysql-slow.log',
                '--default-authentication-plugin=mysql_native_password',
                '--collation-server=utf8mb4_unicode_ci',
                '--character-set-server=utf8mb4'
              );
              await docker(args);
              await waitForMysql();
              console.log();
              break;
            }

            case 'api': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring API is running...');
              let apiExists = await containerExists('tsbx_api');
              if (apiExists) {
                let hasRequiredMounts = false;
                try {
                  const inspect = await execCmd('docker', ['inspect','tsbx_api','--format','{{range .Mounts}}{{println .Destination}}{{end}}'], { silent: true });
                  const mounts = (inspect.stdout || '').split('\n').map(line => line.trim()).filter(Boolean);
                  hasRequiredMounts = mounts.includes('/data/snapshots') && mounts.includes('/var/run/docker.sock');
                } catch (_) {
                  // If inspection fails, assume mounts are correct to avoid unnecessary recreation
                  hasRequiredMounts = true;
                }

                if (!hasRequiredMounts) {
                  console.log(chalk.blue('[INFO] ') + 'Recreating API container to attach required volumes...');
                  try { await docker(['rm','-f','tsbx_api']); } catch (_) {}
                  apiExists = false;
                } else if (await containerRunning('tsbx_api')) {
                  console.log(chalk.green('[SUCCESS] ') + 'API already running');
                  console.log();
                  break;
                } else {
                  await docker(['start','tsbx_api']);
                  console.log(chalk.green('[SUCCESS] ') + 'API started');
                  console.log();
                  break;
                }
              }
              if (!apiExists) {
              const API_IMAGE = await resolveTaskSandboxImage('api','tsbx_api','registry.digitalocean.com/tsbx/tsbx_api', tag);
              const args = ['run','-d',
                '--name','tsbx_api',
                '--network','tsbx_network',
                '-v','tsbx_snapshots_data:/data/snapshots:ro',
                '-v','/var/run/docker.sock:/var/run/docker.sock:ro',
                '-e',`DATABASE_URL=${options.apiDatabaseUrl || 'mysql://tsbx:tsbx@mysql:3306/tsbx'}`,
                '-e',`JWT_SECRET=${options.apiJwtSecret || process.env.JWT_SECRET || 'development-secret-key'}`,
                '-e',`RUST_LOG=${options.apiRustLog || 'info'}`,
                '-e',`TSBX_HOST_NAME=${TSBX_HOST_NAME}`,
                '-e',`TSBX_HOST_URL=${TSBX_HOST_URL}`,
                '-e',`TSBX_INFERENCE_URL=${INFERENCE_URL}`,
                '-e',`TSBX_INFERENCE_API_KEY=${INFERENCE_API_KEY}`,
                '-e',`TSBX_INFERENCE_MODEL=${INFERENCE_MODEL}`,
                '-e',`TSBX_INFERENCE_TEMPLATE=${INFERENCE_TEMPLATE}`,
                ...(options.apiTaskSandboxHost ? ['-e', `TSBX_HOST=${options.apiTaskSandboxHost}`] : []),
                ...(options.apiTaskSandboxPort ? ['-e', `TSBX_PORT=${options.apiTaskSandboxPort}`] : []),
                API_IMAGE
              ];
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'API container started');
              console.log();
              }
              break;
            }

            case 'controller': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring controller service is running...');
              const desiredInferenceUrl = INFERENCE_URL;
              const desiredModel = INFERENCE_MODEL;

              // If container exists, verify env matches; recreate if not
              if (await containerExists('tsbx_controller')) {
                try {
                  const inspect = await execCmd('docker', ['inspect','tsbx_controller','--format','{{range .Config.Env}}{{println .}}{{end}}'], { silent: true });
                  const currentEnv = (inspect.stdout || '').split('\n').filter(Boolean);
                  const envMap = Object.fromEntries(currentEnv.map(e => {
                    const idx = e.indexOf('=');
                    return idx === -1 ? [e, ''] : [e.slice(0, idx), e.slice(idx+1)];
                  }));
                  const currentUrl = envMap['TSBX_INFERENCE_URL'];
                  const currentModel = envMap['TSBX_INFERENCE_MODEL'];
                  const currentKey = envMap['TSBX_INFERENCE_API_KEY'];
                  const needsRecreate =
                    currentUrl !== desiredInferenceUrl ||
                    currentModel !== desiredModel ||
                    currentKey !== INFERENCE_API_KEY;
                  if (needsRecreate) {
                    console.log(chalk.blue('[INFO] ') + 'Recreating controller to apply updated inference configuration');
                    try { await docker(['rm','-f','tsbx_controller']); } catch (_) {}
                  } else if (!(await containerRunning('tsbx_controller'))) {
                    await docker(['start','tsbx_controller']);
                    console.log(chalk.green('[SUCCESS] ') + 'Controller started');
                    console.log();
                    break;
                  } else {
                    console.log(chalk.green('[SUCCESS] ') + 'Controller already running');
                    console.log();
                    break;
                  }
                } catch (e) {
                  // If inspection fails, fall through to create
                }
              }

              const sandboxImage = options.controllerSandboxImage || await resolveTaskSandboxImage('sandbox','tsbx_sandbox','registry.digitalocean.com/tsbx/tsbx_sandbox', tag);
              const controllerDbUrl = options.controllerDatabaseUrl || 'mysql://tsbx:tsbx@mysql:3306/tsbx';
              const controllerJwt = options.controllerJwtSecret || process.env.JWT_SECRET || 'development-secret-key';
              const controllerRustLog = options.controllerRustLog || 'info';
              const args = ['run','-d',
                '--name','tsbx_controller',
                '--network','tsbx_network',
                '-v','/var/run/docker.sock:/var/run/docker.sock',
                '-v','tsbx_snapshots_data:/data/snapshots',
                '-e',`DATABASE_URL=${controllerDbUrl}`,
                '-e',`JWT_SECRET=${controllerJwt}`,
                '-e',`TSBX_INFERENCE_URL=${desiredInferenceUrl}`,
                '-e',`TSBX_INFERENCE_API_KEY=${INFERENCE_API_KEY}`,
                '-e',`TSBX_INFERENCE_MODEL=${desiredModel}`,
                '-e',`TSBX_INFERENCE_TEMPLATE=${INFERENCE_TEMPLATE}`,
                '-e',`TSBX_HOST_NAME=${TSBX_HOST_NAME}`,
                '-e',`TSBX_HOST_URL=${TSBX_HOST_URL}`,
                '-e',`SANDBOX_IMAGE=${sandboxImage}`,
                '-e',`SANDBOX_CPU_LIMIT=${options.controllerSandboxCpuLimit || '0.5'}`,
                '-e',`SANDBOX_MEMORY_LIMIT=${(getOptionSource('controllerSandboxMemoryLimit')==='cli' ? options.controllerSandboxMemoryLimit : (process.env.SANDBOX_MEMORY_LIMIT || options.controllerSandboxMemoryLimit || '536870912'))}`,
                '-e',`SANDBOX_DISK_LIMIT=${options.controllerSandboxDiskLimit || '1073741824'}`,
                '-e',`RUST_LOG=${controllerRustLog}`
              ];
              // append image ref last
              args.push(await resolveTaskSandboxImage('controller','tsbx_controller','registry.digitalocean.com/tsbx/tsbx_controller', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Controller service container started');
              console.log();
              break;
            }

            case 'operator': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring Operator UI is running...');

              if (!process.env.TSBX_HOST_NAME || !process.env.TSBX_HOST_URL) {
                console.error(chalk.red('[ERROR] ') + 'TSBX_HOST_NAME and TSBX_HOST_URL must be set before starting tsbx_operator.');
                process.exit(1);
              }

              if (await containerExists('tsbx_operator')) {
                // If container exists, ensure it matches the desired image; recreate if not
                const running = await containerRunning('tsbx_operator');
                const currentId = await containerImageId('tsbx_operator');
              const desiredId = await imageId(await resolveTaskSandboxImage('operator','tsbx_operator','registry.digitalocean.com/tsbx/tsbx_operator', tag));
                if (currentId && desiredId && currentId !== desiredId) {
                  console.log(chalk.blue('[INFO] ') + 'Operator image changed; recreating container to apply updates...');
                  try { await docker(['rm','-f','tsbx_operator']); } catch (_) {}
                } else if (running) {
                  console.log(chalk.green('[SUCCESS] ') + 'Operator already running');
                  console.log();
                  break;
                } else if (!running && currentId && desiredId && currentId === desiredId) {
                  await docker(['start','tsbx_operator']);
                  console.log(chalk.green('[SUCCESS] ') + 'Operator started');
                  console.log();
                  break;
                } else {
                  // fallthrough to create
                }
              }
              const args = ['run'];
              if (detached) args.push('-d');
              args.push(
                '--name','tsbx_operator',
                '--network','tsbx_network',
                '-e',`TSBX_HOST_NAME=${TSBX_HOST_NAME}`,
                '-e',`TSBX_HOST_URL=${TSBX_HOST_URL}`
              );
              args.push(await resolveTaskSandboxImage('operator','tsbx_operator','registry.digitalocean.com/tsbx/tsbx_operator', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Operator UI container started');
              console.log();
              break;
            }

            case 'gateway': {
              const hostPortNumber = parseInt(hostPort, 10);
              const inspectGatewayHostPort = async () => {
                try {
                  const res = await docker(
                    [
                      'inspect',
                      'tsbx_gateway',
                      '--format',
                      '{{range $k,$v := .HostConfig.PortBindings}}{{if eq $k "80/tcp"}}{{(index $v 0).HostPort}}{{end}}{{end}}',
                    ],
                    { silent: true }
                  );
                  return (res.stdout || '').trim();
                } catch (_) {
                  return '';
                }
              };
              console.log(chalk.blue('[INFO] ') + `Ensuring gateway (NGINX) is running on port ${hostPort}...`);
              if (await containerRunning('tsbx_gateway')) {
                const boundPort = await inspectGatewayHostPort();
                console.log(chalk.green('[SUCCESS] ') + `Gateway already running (port ${boundPort || hostPort})`);
                console.log();
                break;
              }
              if (await containerExists('tsbx_gateway')) {
                const boundPort = await inspectGatewayHostPort();
                if (boundPort && boundPort !== hostPort) {
                  console.log(chalk.blue('[INFO] ') + `Existing gateway is bound to port ${boundPort}; recreating container for port ${hostPort}.`);
                  try {
                    await docker(['rm', '-f', 'tsbx_gateway']);
                  } catch (e) {
                    console.log(chalk.yellow('[WARNING] ') + `Failed to remove existing gateway container: ${e.message}`);
                    console.log();
                    break;
                  }
                } else {
                  await docker(['start','tsbx_gateway']);
                  console.log(chalk.green('[SUCCESS] ') + `Gateway started (port ${boundPort || hostPort})`);
                  console.log();
                  break;
                }
              }
              if (await portInUse(hostPortNumber)) {
                console.log(chalk.yellow('[WARNING] ') + `Port ${hostPort} is already in use on the host. Gateway will fail to bind.`);
              }
              const args = ['run'];
              if (detached) args.push('-d');
              args.push('--name','tsbx_gateway','--network','tsbx_network','-p',`${hostPort}:80`);
              args.push(await resolveTaskSandboxImage('gateway','tsbx_gateway','registry.digitalocean.com/tsbx/tsbx_gateway', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + `Gateway container started (port ${hostPort})`);
              console.log();
              break;
            }

            default:
              console.log(chalk.yellow('[WARNING] ') + `Unknown component: ${comp}. Skipping...`);
          }
        }

        // Show status summary
        console.log(chalk.blue('[INFO] ') + 'Checking running services...');
        console.log();
        let status = '';
        try {
          const res = await docker(['ps','--filter','name=tsbx_','--format','table {{.Names}}\t{{.Status}}\t{{.Ports}}'], { silent: true });
          status = res.stdout;
        } catch(_) {}
        if (status && status.trim()) {
          console.log(status);
          console.log();
          console.log(chalk.green('[SUCCESS] ') + '🎉 TaskSandbox services are now running!');
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Service URLs:');
          try {
            const g = await docker(['ps','--filter','name=tsbx_gateway','--format','{{.Names}}'], { silent: true });
            if (g.stdout.trim()) {
              const gatewayBaseUrl = hostPort === '80' ? TSBX_HOST_URL : withPort(TSBX_HOST_URL, hostPort);
              console.log(`  • Gateway: ${gatewayBaseUrl}/`);
              console.log(`  • Operator UI: ${gatewayBaseUrl}/`);
              console.log(`  • API via Gateway: ${gatewayBaseUrl}/api`);
            } else {
              console.log('  • Gateway not running; API and Operator are not exposed on host ports.');
            }
          } catch(_) {}
          try {
            const m = await docker(['ps','--filter','name=mysql','--format','{{.Names}}'], { silent: true });
            if (m.stdout.trim()) {
              console.log('  • MySQL Port: 3307');
            }
          } catch(_) {}
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Next steps:');
          console.log('  • Check logs: docker logs tsbx_api -f');
          console.log('  • Authenticate: tsbx login -u admin -p admin');
          console.log('  • Check version: tsbx api version');
          console.log('  • Start sandbox: tsbx sandbox create');
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Container management:');
          console.log("  • Stop services: tsbx stop");
          console.log("  • View logs: docker logs <container_name>");
          console.log("  • Check status: docker ps --filter 'name=tsbx_'");
        } else {
          console.error(chalk.red('[ERROR] ') + 'No TaskSandbox containers are running');
          process.exit(1);
        }
      } catch (error) {
        console.error(chalk.red('[ERROR] ') + (error && error.message ? error.message : String(error)));
        process.exit(1);
      }
    });
};
