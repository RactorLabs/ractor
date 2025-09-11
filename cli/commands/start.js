const chalk = require('chalk');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

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
    await docker(['network', 'inspect', 'raworc_network'], { silent: true });
  } catch (_) {
    await docker(['network', 'create', 'raworc_network']);
  }
}

async function ensureVolumes() {
  for (const v of ['mysql_data', 'raworc_content_data', 'ollama_data', 'raworc_api_data', 'raworc_operator_data', 'raworc_controller_data']) {
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
    .argument('[components...]', 'Components to start. Default: all. Allowed: mysql, ollama, api, controller, operator, content, gateway', [])
    .option('-p, --pull', 'Pull base images (mysql) before starting')
    .option('-d, --detached', 'Run in detached mode', true)
    .option('-f, --foreground', 'Run MySQL in foreground mode')
    .option('--require-gpu', 'Require GPU for Ollama (fail if missing)')
    .option('--ollama-cpus <cpus>', 'CPUs for Ollama (e.g., 4)')
    .option('--ollama-memory <mem>', 'Memory for Ollama (e.g., 16g)')
    .option('--ollama-shm-size <size>', 'Shared memory for Ollama (e.g., 16g)')
    .option('--ollama-enable-gpu', 'Enable GPU for Ollama (default true)')
    .option('--no-ollama-enable-gpu', 'Disable GPU for Ollama')
    .option('--ollama-model <model>', 'Ollama model name', 'gpt-oss:20b')
    .option('--ollama-keep-alive <dur>', 'Ollama keep alive duration', '1h')
    .option('--ollama-context-length <tokens>', 'Ollama context length in tokens', '131072')
    // MySQL options
    .option('--mysql-port <port>', 'Host port for MySQL', '3307')
    .option('--mysql-root-password <pw>', 'MySQL root password', 'root')
    .option('--mysql-database <db>', 'MySQL database name', 'raworc')
    .option('--mysql-user <user>', 'MySQL user', 'raworc')
    .option('--mysql-password <pw>', 'MySQL user password', 'raworc')
    // API options
    .option('--api-database-url <url>', 'API DATABASE_URL', 'mysql://raworc:raworc@mysql:3306/raworc')
    .option('--api-jwt-secret <secret>', 'API JWT_SECRET')
    .option('--api-rust-log <level>', 'API RUST_LOG', 'info')
    .option('--api-raworc-host <host>', 'API RAWORC_HOST')
    .option('--api-raworc-port <port>', 'API RAWORC_PORT')
    .option('--api-api-port <port>', 'Host port for API (maps to 9000)', '9000')
    .option('--api-public-port <port>', 'Host port for public content (maps to 8000)', '8000')
    // Controller options
    .option('--controller-database-url <url>', 'Controller DATABASE_URL', 'mysql://raworc:raworc@mysql:3306/raworc')
    .option('--controller-jwt-secret <secret>', 'Controller JWT_SECRET')
    .option('--controller-rust-log <level>', 'Controller RUST_LOG', 'info')
    .option('--controller-ollama-host <url>', 'Controller OLLAMA_HOST (overrides autodetection)')
    .option('--controller-ollama-model <model>', 'Controller OLLAMA_MODEL')
    .addHelpText('after', '\n' +
      'Notes:\n' +
      '  • Starts each component if stopped, or creates it if missing.\n' +
      '  • Does not stop or remove any containers.\n' +
      '  • MySQL container name is "mysql"; Ollama container name is "ollama".\n' +
      '\nExamples:\n' +
      '  $ raworc start                                # Start full stack\n' +
      '  $ raworc start api controller                 # Start API + controller\n' +
      '  $ raworc start mysql                          # Ensure MySQL is up\n')
    .option('--controller-agent-image <image>', 'Controller AGENT_IMAGE')
    .option('--controller-agent-cpu-limit <n>', 'Controller AGENT_CPU_LIMIT', '0.5')
    .option('--controller-agent-memory-limit <bytes>', 'Controller AGENT_MEMORY_LIMIT', '536870912')
    .option('--controller-agent-disk-limit <bytes>', 'Controller AGENT_DISK_LIMIT', '1073741824')
    .action(async (components, options) => {
      try {
        const detached = options.foreground ? false : (options.detached !== false);
        const tag = readProjectVersionOrLatest();

        // Resolve host branding and URL only here (script-level default allowed)
        const RAWORC_HOST_NAME = process.env.RAWORC_HOST_NAME || 'Raworc';
        const RAWORC_HOST_URL = (process.env.RAWORC_HOST_URL || 'http://localhost').replace(/\/$/, '');

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

        async function resolveRaworcImage(component, localShortName, remoteRepo, tag) {
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

        console.log(chalk.blue('[INFO] ') + 'Starting Raworc services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Image tag: ${tag}`);
        console.log(chalk.blue('[INFO] ') + `Pull base images: ${!!options.pull}`);
        console.log(chalk.blue('[INFO] ') + `Detached mode: ${detached}`);
        console.log(chalk.blue('[INFO] ') + `Require GPU for Ollama: ${!!options.requireGpu}`);

        if (!components || components.length === 0) {
          components = ['mysql', 'ollama', 'api', 'operator', 'content', 'controller', 'gateway'];
        }

        // Enforce startup order: mysql → ollama → api → controller
        // In particular, ensure api starts before controller when both are requested.
        const desiredOrder = ['mysql', 'ollama', 'api', 'operator', 'content', 'controller', 'gateway'];
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
          // Raworc images are resolved lazily when each component starts.
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
                '--network','raworc_network',
                '-p', `${String(options.mysqlPort || '3307')}:3306`,
                '-v','mysql_data:/var/lib/mysql',
                '-e',`MYSQL_ROOT_PASSWORD=${options.mysqlRootPassword || 'root'}`,
                '-e',`MYSQL_DATABASE=${options.mysqlDatabase || 'raworc'}`,
                '-e',`MYSQL_USER=${options.mysqlUser || 'raworc'}`,
                '-e',`MYSQL_PASSWORD=${options.mysqlPassword || 'raworc'}`,
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

            case 'ollama': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring Ollama runtime is running...');
              if (await containerRunning('ollama')) {
                console.log(chalk.green('[SUCCESS] ') + 'Ollama already running');
                // Ensure requested model is available even when container pre-exists
                const effectiveModel = (() => {
                  const src = getOptionSource('ollamaModel');
                  if (src === 'cli') return options.ollamaModel;
                  return process.env.OLLAMA_MODEL || options.ollamaModel || 'gpt-oss:20b';
                })();
                if (effectiveModel) {
                  console.log(chalk.blue('[INFO] ') + `Pulling ${effectiveModel} model (if needed)...`);
                  try { await docker(['exec','ollama','ollama','pull', effectiveModel], { silent: true }); console.log(chalk.green('[SUCCESS] ') + `${effectiveModel} model available`);} catch(_) { console.log(chalk.yellow('[WARNING] ') + `Failed to pull ${effectiveModel}. You may need to pull manually.`); }
                }
                console.log();
                break;
              }
              if (await containerExists('ollama')) {
                await docker(['start','ollama']);
                console.log(chalk.green('[SUCCESS] ') + 'Ollama started');
                // Ensure requested model is available even when container pre-exists
                const effectiveModel = (() => {
                  const src = getOptionSource('ollamaModel');
                  if (src === 'cli') return options.ollamaModel;
                  return process.env.OLLAMA_MODEL || options.ollamaModel || 'gpt-oss:20b';
                })();
                if (effectiveModel) {
                  console.log(chalk.blue('[INFO] ') + `Pulling ${effectiveModel} model (if needed)...`);
                  try { await docker(['exec','ollama','ollama','pull', effectiveModel], { silent: true }); console.log(chalk.green('[SUCCESS] ') + `${effectiveModel} model available`);} catch(_) { console.log(chalk.yellow('[WARNING] ') + `Failed to pull ${effectiveModel}. You may need to pull manually.`); }
                }
                console.log();
                break;
              }

              // Determine GPU availability and flags
              // GPU enable: flags > env > default(true). Env can be OLLAMA_ENABLE_GPU or OLLAMA_NO_GPU
              let OLLAMA_ENABLE_GPU;
              if (getOptionSource('ollamaEnableGpu') === 'cli') {
                OLLAMA_ENABLE_GPU = !!options.ollamaEnableGpu;
              } else {
                // honor NO_GPU first if set
                const noGpu = envBool('OLLAMA_NO_GPU', false);
                if (noGpu) {
                  OLLAMA_ENABLE_GPU = false;
                } else if (process.env.OLLAMA_ENABLE_GPU !== undefined) {
                  OLLAMA_ENABLE_GPU = envBool('OLLAMA_ENABLE_GPU', true);
                } else {
                  OLLAMA_ENABLE_GPU = true;
                }
              }
              const REQUIRE_GPU = !!options.requireGpu;
              let gpuAvailable = false;
              try {
                const r1 = await execCmd('docker', ['info','--format','{{json .Runtimes}}'], { silent: true });
                const r2 = await execCmd('docker', ['info','--format','{{json .DefaultRuntime}}'], { silent: true });
                if (/nvidia/i.test(r1.stdout) || /nvidia/i.test(r2.stdout)) gpuAvailable = true;
              } catch (_) {}
              let gpuFlags = [];
              let cpuEnv = [];
              if (OLLAMA_ENABLE_GPU) {
                if (gpuAvailable) {
                  gpuFlags = ['--gpus','all'];
                  console.log(chalk.blue('[INFO] ') + 'GPU enabled for Ollama');
                } else {
                  if (REQUIRE_GPU) {
                    console.error(chalk.red('[ERROR] ') + 'GPU required for Ollama, but Docker GPU runtime is not available.');
                    console.error(chalk.red('[ERROR] ') + 'Install NVIDIA drivers + NVIDIA Container Toolkit, or omit --require-gpu.');
                    process.exit(1);
                  } else {
                    console.log(chalk.yellow('[WARNING] ') + 'GPU requested but not available; falling back to CPU.');
                    OLLAMA_ENABLE_GPU = false;
                  }
                }
              }
              if (!OLLAMA_ENABLE_GPU) {
                cpuEnv = ['-e','OLLAMA_NO_GPU=1'];
                console.log(chalk.blue('[INFO] ') + 'Running Ollama in CPU-only mode');
              }

              // Resource flags (flags > env > defaults)
              const cpus = preferEnv('ollamaCpus', 'OLLAMA_CPUS', undefined);
              const cpuFlag = cpus ? ['--cpus', cpus] : [];
              let mem = preferEnv('ollamaMemory', 'OLLAMA_MEMORY', '16g');
              let shm = preferEnv('ollamaShmSize', 'OLLAMA_SHM_SIZE', '16g');
              if (getOptionSource('ollamaMemory') !== 'cli' && process.env.OLLAMA_MEMORY === undefined) console.log(chalk.blue('[INFO] ') + `No OLLAMA_MEMORY set; defaulting to ${mem}`);
              if (getOptionSource('ollamaShmSize') !== 'cli' && process.env.OLLAMA_SHM_SIZE === undefined) console.log(chalk.blue('[INFO] ') + `No OLLAMA_SHM_SIZE set; defaulting to ${shm}`);
              const contextLength = (() => {
                const src = getOptionSource('ollamaContextLength');
                if (src === 'cli') return String(options.ollamaContextLength);
                if (process.env.OLLAMA_CONTEXT_LENGTH) return String(process.env.OLLAMA_CONTEXT_LENGTH);
                if (process.env.OLLAMA_NUM_CTX) return String(process.env.OLLAMA_NUM_CTX);
                return String(options.ollamaContextLength || '131072');
              })();
              console.log(chalk.blue('[INFO] ') + `Ollama context length: ${contextLength} tokens (128k)`);
              const memFlag = ['--memory', mem, '--memory-swap', mem];
              const shmFlag = ['--shm-size', shm];

              // Host port mapping if free
              const hostPublish = !(await portInUse(11434));
              if (!hostPublish) console.log(chalk.yellow('[WARNING] ') + 'Host port 11434 in use; starting without host port mapping');

              const args = ['run','-d',
                '--name','ollama',
                '--network','raworc_network',
              ];
              if (hostPublish) args.push('-p','11434:11434');
              args.push(
                '-v','ollama_data:/root/.ollama',
                '-v','ollama_data:/var/log/ollama',
                '-e',`OLLAMA_KEEP_ALIVE=${preferEnv('ollamaKeepAlive','OLLAMA_KEEP_ALIVE','1h')}`,
                '-e',`OLLAMA_CONTEXT_LENGTH=${contextLength}`,
                '-e',`OLLAMA_NUM_CTX=${contextLength}`,
                ...cpuEnv,
                ...gpuFlags,
                ...cpuFlag,
                ...memFlag,
                ...shmFlag,
                '--entrypoint','/bin/sh',
                'ollama/ollama:latest',
                '-lc',
                'mkdir -p /var/log/ollama; exec ollama serve 2>&1 | tee -a /var/log/ollama/ollama.log'
              );

              await docker(args);

              // Wait until ready (new container only)
              let timeoutMs = 600000; // 10 minutes to allow large model loads
              const start = Date.now();
              if (hostPublish) {
                console.log(chalk.blue('[INFO] ') + 'Waiting for Ollama to be ready on host :11434...');
                while (Date.now() - start < timeoutMs) {
              try { await execCmd('bash',['-lc',`curl -fsS ${withPort(RAWORC_HOST_URL,11434)}/api/tags >/dev/null`]); break; } catch(_) {}
                  await new Promise(r=>setTimeout(r,2000));
                }
              } else {
                console.log(chalk.blue('[INFO] ') + 'Waiting for Ollama container to be ready...');
                while (Date.now() - start < timeoutMs) {
                  try { await docker(['exec','ollama','ollama','list'], { silent: true }); break; } catch(_) {}
                  await new Promise(r=>setTimeout(r,2000));
                }
              }
              if (Date.now() - start >= timeoutMs) {
                throw new Error('Ollama did not become ready in time');
              }
              console.log(chalk.green('[SUCCESS] ') + 'Ollama is ready');

              // Ensure model available (best-effort)
              const effectiveModel = (() => {
                const src = getOptionSource('ollamaModel');
                if (src === 'cli') return options.ollamaModel;
                return process.env.OLLAMA_MODEL || options.ollamaModel || 'gpt-oss:20b';
              })();
              console.log(chalk.blue('[INFO] ') + `Pulling ${effectiveModel} model (if needed)...`);
              try { await docker(['exec','ollama','ollama','pull', effectiveModel], { silent: true }); console.log(chalk.green('[SUCCESS] ') + `${effectiveModel} model available`);} catch(_) { console.log(chalk.yellow('[WARNING] ') + `Failed to pull ${effectiveModel}. You may need to pull manually.`); }
              console.log();
              break;
            }

            case 'api': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring API is running...');
              if (await containerRunning('raworc_api')) { console.log(chalk.green('[SUCCESS] ') + 'API already running'); console.log(); break; }
              if (await containerExists('raworc_api')) {
                await docker(['start','raworc_api']);
                console.log(chalk.green('[SUCCESS] ') + 'API started');
                console.log();
                break;
              }
              const API_IMAGE = await resolveRaworcImage('api','raworc_api','raworc/raworc_api', tag);
              const args = ['run','-d',
                '--name','raworc_api',
                '--network','raworc_network',
                '-v', 'raworc_api_data:/app/logs',
                '-e',`DATABASE_URL=${options.apiDatabaseUrl || 'mysql://raworc:raworc@mysql:3306/raworc'}`,
                '-e',`JWT_SECRET=${options.apiJwtSecret || process.env.JWT_SECRET || 'development-secret-key'}`,
                '-e',`RUST_LOG=${options.apiRustLog || 'info'}`,
                '-e',`RAWORC_HOST_NAME=${RAWORC_HOST_NAME}`,
                '-e',`RAWORC_HOST_URL=${RAWORC_HOST_URL}`,
                ...(options.apiRaworcHost ? ['-e', `RAWORC_HOST=${options.apiRaworcHost}`] : []),
                ...(options.apiRaworcPort ? ['-e', `RAWORC_PORT=${options.apiRaworcPort}`] : []),
                API_IMAGE
              ];
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'API container started');
              console.log();
              break;
            }

            case 'controller': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring controller service is running...');
              if (await containerRunning('raworc_controller')) { console.log(chalk.green('[SUCCESS] ') + 'Controller already running'); console.log(); break; }
              if (await containerExists('raworc_controller')) {
                await docker(['start','raworc_controller']);
                console.log(chalk.green('[SUCCESS] ') + 'Controller started');
                console.log();
                break;
              }
              // Default OLLAMA_HOST: prefer internal container if running
              let OLLAMA_HOST = options.controllerOllamaHost || process.env.OLLAMA_HOST;
              try {
                const res = await docker(['ps','-q','--filter','name=ollama'], { silent: true });
                const hasOllama = !!res.stdout.trim();
                if (!OLLAMA_HOST) OLLAMA_HOST = hasOllama ? 'http://ollama:11434' : 'http://host.docker.internal:11434';
              } catch (_) {
                if (!OLLAMA_HOST) OLLAMA_HOST = 'http://host.docker.internal:11434';
              }

              const agentImage = options.controllerAgentImage || await resolveRaworcImage('agent','raworc_agent','raworc/raworc_agent', tag);
              const controllerDbUrl = options.controllerDatabaseUrl || 'mysql://raworc:raworc@mysql:3306/raworc';
              const controllerJwt = options.controllerJwtSecret || process.env.JWT_SECRET || 'development-secret-key';
              const controllerRustLog = options.controllerRustLog || 'info';
              const model = (() => {
                const srcCtrl = getOptionSource('controllerOllamaModel');
                const srcOllama = getOptionSource('ollamaModel');
                if (srcCtrl === 'cli') return options.controllerOllamaModel;
                if (srcOllama === 'cli') return options.ollamaModel;
                return process.env.OLLAMA_MODEL || options.controllerOllamaModel || options.ollamaModel || 'gpt-oss:20b';
              })();
              const args = ['run','-d',
                '--name','raworc_controller',
                '--network','raworc_network',
                '-v','/var/run/docker.sock:/var/run/docker.sock',
                '-v','raworc_controller_data:/app/logs',
                '-e',`DATABASE_URL=${controllerDbUrl}`,
                '-e',`JWT_SECRET=${controllerJwt}`,
                '-e',`OLLAMA_HOST=${OLLAMA_HOST}`,
                '-e',`OLLAMA_MODEL=${model}`,
                '-e',`RAWORC_HOST_NAME=${RAWORC_HOST_NAME}`,
                '-e',`RAWORC_HOST_URL=${RAWORC_HOST_URL}`,
                '-e',`AGENT_IMAGE=${agentImage}`,
                '-e',`AGENT_CPU_LIMIT=${options.controllerAgentCpuLimit || '0.5'}`,
                '-e',`AGENT_MEMORY_LIMIT=${(getOptionSource('controllerAgentMemoryLimit')==='cli' ? options.controllerAgentMemoryLimit : (process.env.AGENT_MEMORY_LIMIT || options.controllerAgentMemoryLimit || '536870912'))}`,
                '-e',`AGENT_DISK_LIMIT=${options.controllerAgentDiskLimit || '1073741824'}`,
                '-e',`RUST_LOG=${controllerRustLog}`
              ];
              // append image ref last
              args.push(await resolveRaworcImage('controller','raworc_controller','raworc/raworc_controller', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Controller service container started');
              console.log();
              break;
            }

            case 'operator': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring Operator UI is running...');
              if (await containerExists('raworc_operator')) {
                // If container exists, ensure it matches the desired image; recreate if not
                const running = await containerRunning('raworc_operator');
                const currentId = await containerImageId('raworc_operator');
                const desiredId = await imageId(await resolveRaworcImage('operator','raworc_operator','raworc/raworc_operator', tag));
                if (currentId && desiredId && currentId !== desiredId) {
                  console.log(chalk.blue('[INFO] ') + 'Operator image changed; recreating container to apply updates...');
                  try { await docker(['rm','-f','raworc_operator']); } catch (_) {}
                } else if (running) {
                  console.log(chalk.green('[SUCCESS] ') + 'Operator already running');
                  console.log();
                  break;
                } else if (!running && currentId && desiredId && currentId === desiredId) {
                  await docker(['start','raworc_operator']);
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
                '--name','raworc_operator',
                '--network','raworc_network',
                '-v','raworc_content_data:/content',
                '-v','raworc_operator_data:/app/logs',
                '-e',`RAWORC_HOST_NAME=${RAWORC_HOST_NAME}`,
                '-e',`RAWORC_HOST_URL=${RAWORC_HOST_URL}`
              );
              args.push(await resolveRaworcImage('operator','raworc_operator','raworc/raworc_operator', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Operator UI container started');
              console.log();
              break;
            }

            case 'content': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring Content service is running...');
              if (await containerRunning('raworc_content')) { console.log(chalk.green('[SUCCESS] ') + 'Content already running'); console.log(); break; }
              if (await containerExists('raworc_content')) {
                await docker(['start','raworc_content']);
                console.log(chalk.green('[SUCCESS] ') + 'Content started');
                console.log();
                break;
              }
              const CONTENT_IMAGE = await resolveRaworcImage('content','raworc_content','raworc/raworc_content', tag);
              const args = ['run'];
              if (detached) args.push('-d');
              args.push('--name','raworc_content','--network','raworc_network','-v','raworc_content_data:/content', CONTENT_IMAGE);
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Content service container started');
              console.log();
              break;
            }

            case 'gateway': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring gateway (NGINX) is running on port 80...');
              if (await containerRunning('raworc_gateway')) { console.log(chalk.green('[SUCCESS] ') + 'Gateway already running'); console.log(); break; }
              if (await containerExists('raworc_gateway')) {
                await docker(['start','raworc_gateway']);
                console.log(chalk.green('[SUCCESS] ') + 'Gateway started');
                console.log();
                break;
              }
              if (await portInUse(80)) {
                console.log(chalk.yellow('[WARNING] ') + 'Port 80 is already in use on the host. Gateway will fail to bind.');
              }
              const args = ['run'];
              if (detached) args.push('-d');
              args.push('--name','raworc_gateway','--network','raworc_network','-p','80:80');
              args.push(await resolveRaworcImage('gateway','raworc_gateway','raworc/raworc_gateway', tag));
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'Gateway container started (port 80)');
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
          const res = await docker(['ps','--filter','name=raworc_','--format','table {{.Names}}\t{{.Status}}\t{{.Ports}}'], { silent: true });
          status = res.stdout;
        } catch(_) {}
        if (status && status.trim()) {
          console.log(status);
          console.log();
          console.log(chalk.green('[SUCCESS] ') + '🎉 Raworc services are now running!');
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Service URLs:');
          try {
            const g = await docker(['ps','--filter','name=raworc_gateway','--format','{{.Names}}'], { silent: true });
            if (g.stdout.trim()) {
              console.log(`  • Gateway: ${RAWORC_HOST_URL}/`);
              console.log(`  • Operator UI: ${RAWORC_HOST_URL}/`);
              console.log(`  • API via Gateway: ${RAWORC_HOST_URL}/api`);

              console.log(`  • Content: ${RAWORC_HOST_URL}/content`);
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
          console.log('  • Check logs: docker logs raworc_api -f');
          console.log('  • Authenticate: raworc login -u admin -p admin');
          console.log('  • Check version: raworc api version');
          console.log('  • Start agent: raworc agent create');
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Container management:');
          console.log("  • Stop services: raworc stop");
          console.log("  • View logs: docker logs <container_name>");
          console.log("  • Check status: docker ps --filter 'name=raworc_'");
        } else {
          console.error(chalk.red('[ERROR] ') + 'No Raworc containers are running');
          process.exit(1);
        }
      } catch (error) {
        console.error(chalk.red('[ERROR] ') + (error && error.message ? error.message : String(error)));
        process.exit(1);
      }
    });
};
