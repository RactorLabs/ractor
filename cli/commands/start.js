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
  for (const v of ['raworc_mysql_data', 'raworc_content_data', 'raworc_ollama_data', 'raworc_logs']) {
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
      await docker(['exec', 'raworc_mysql', 'mysqladmin', 'ping', '-h', 'localhost', '-u', 'root', '-proot'], { silent: true });
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
    .description('Start Raworc services (idempotent): starts only missing or stopped components')
    .argument('[components...]', 'Components to start (mysql, ollama, server, controller). Default: all', [])
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
    // Server options
    .option('--server-database-url <url>', 'Server DATABASE_URL', 'mysql://raworc:raworc@raworc_mysql:3306/raworc')
    .option('--server-jwt-secret <secret>', 'Server JWT_SECRET')
    .option('--server-rust-log <level>', 'Server RUST_LOG', 'info')
    .option('--server-raworc-host <host>', 'Server RAWORC_HOST')
    .option('--server-raworc-port <port>', 'Server RAWORC_PORT')
    .option('--server-api-port <port>', 'Host port for API (maps to 9000)', '9000')
    .option('--server-public-port <port>', 'Host port for public content (maps to 8000)', '8000')
    // Controller options
    .option('--controller-database-url <url>', 'Controller DATABASE_URL', 'mysql://raworc:raworc@raworc_mysql:3306/raworc')
    .option('--controller-jwt-secret <secret>', 'Controller JWT_SECRET')
    .option('--controller-rust-log <level>', 'Controller RUST_LOG', 'info')
    .option('--controller-ollama-host <url>', 'Controller OLLAMA_HOST (overrides autodetection)')
    .option('--controller-ollama-model <model>', 'Controller OLLAMA_MODEL')
    .option('--controller-agent-image <image>', 'Controller AGENT_IMAGE')
    .option('--controller-agent-cpu-limit <n>', 'Controller AGENT_CPU_LIMIT', '0.5')
    .option('--controller-agent-memory-limit <bytes>', 'Controller AGENT_MEMORY_LIMIT', '536870912')
    .option('--controller-agent-disk-limit <bytes>', 'Controller AGENT_DISK_LIMIT', '1073741824')
    .action(async (components, options) => {
      try {
        const detached = options.foreground ? false : (options.detached !== false);
        const tag = readProjectVersionOrLatest();

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

        const SERVER_IMAGE = await resolveRaworcImage('server','raworc_server','raworc/raworc_server', tag);
        const CONTROLLER_IMAGE = await resolveRaworcImage('controller','raworc_controller','raworc/raworc_controller', tag);
        const AGENT_IMAGE = await resolveRaworcImage('agent','raworc_agent','raworc/raworc_agent', tag);
        const OPERATOR_IMAGE = await resolveRaworcImage('operator','raworc_operator','raworc/raworc_operator', tag);
        const GATEWAY_IMAGE = await resolveRaworcImage('gateway','raworc_gateway','raworc/raworc_gateway', tag);

        console.log(chalk.blue('[INFO] ') + 'Starting Raworc services with direct Docker management');
        console.log(chalk.blue('[INFO] ') + `Image tag: ${tag}`);
        console.log(chalk.blue('[INFO] ') + `Pull base images: ${!!options.pull}`);
        console.log(chalk.blue('[INFO] ') + `Detached mode: ${detached}`);
        console.log(chalk.blue('[INFO] ') + `Require GPU for Ollama: ${!!options.requireGpu}`);

        if (!components || components.length === 0) {
          components = ['mysql', 'ollama', 'server', 'operator', 'content', 'controller', 'gateway'];
        }

        // Enforce startup order: mysql → ollama → server → controller
        // In particular, ensure server starts before controller when both are requested.
        const desiredOrder = ['mysql', 'ollama', 'server', 'operator', 'content', 'controller', 'gateway'];
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
          // Raworc images are already resolved and pulled above if missing.
          console.log();
        }

        await ensureNetwork();
        await ensureVolumes();
        console.log();

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
              if (await containerRunning('raworc_mysql')) { console.log(chalk.green('[SUCCESS] ') + 'MySQL already running'); console.log(); break; }
              if (await containerExists('raworc_mysql')) {
                await docker(['start','raworc_mysql']);
                console.log(chalk.green('[SUCCESS] ') + 'MySQL started');
                console.log();
                break;
              }
              const args = ['run'];
              if (detached) args.push('-d');
              args.push(
                '--name','raworc_mysql',
                '--network','raworc_network',
                '-p', `${String(options.mysqlPort || '3307')}:3306`,
                '-v','raworc_mysql_data:/var/lib/mysql',
                '-e',`MYSQL_ROOT_PASSWORD=${options.mysqlRootPassword || 'root'}`,
                '-e',`MYSQL_DATABASE=${options.mysqlDatabase || 'raworc'}`,
                '-e',`MYSQL_USER=${options.mysqlUser || 'raworc'}`,
                '-e',`MYSQL_PASSWORD=${options.mysqlPassword || 'raworc'}`,
                '--health-cmd','mysqladmin ping -h localhost -u root -proot',
                '--health-interval','10s',
                '--health-timeout','5s',
                '--health-retries','5',
                'mysql:8.0',
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
              if (await containerRunning('raworc_ollama')) { console.log(chalk.green('[SUCCESS] ') + 'Ollama already running'); console.log(); break; }
              if (await containerExists('raworc_ollama')) {
                await docker(['start','raworc_ollama']);
                console.log(chalk.green('[SUCCESS] ') + 'Ollama started');
                console.log();
                break;
              }

              // Determine GPU availability and flags
              let OLLAMA_ENABLE_GPU = (options.ollamaEnableGpu !== undefined) ? !!options.ollamaEnableGpu : true;
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

              // Resource flags
              const cpuFlag = options.ollamaCpus ? ['--cpus', options.ollamaCpus] : [];
              let mem = options.ollamaMemory || '16g';
              let shm = options.ollamaShmSize || '16g';
              if (!options.ollamaMemory) console.log(chalk.blue('[INFO] ') + `No OLLAMA_MEMORY set; defaulting to ${mem}`);
              if (!options.ollamaShmSize) console.log(chalk.blue('[INFO] ') + `No OLLAMA_SHM_SIZE set; defaulting to ${shm}`);
              const contextLength = options.ollamaContextLength || '131072';
              console.log(chalk.blue('[INFO] ') + `Ollama context length: ${contextLength} tokens (128k)`);
              const memFlag = ['--memory', mem, '--memory-swap', mem];
              const shmFlag = ['--shm-size', shm];

              // Host port mapping if free
              const hostPublish = !(await portInUse(11434));
              if (!hostPublish) console.log(chalk.yellow('[WARNING] ') + 'Host port 11434 in use; starting without host port mapping');

              const args = ['run','-d',
                '--name','raworc_ollama',
                '--network','raworc_network',
              ];
              if (hostPublish) args.push('-p','11434:11434');
              args.push(
                '-v','raworc_ollama_data:/root/.ollama',
                '-e',`OLLAMA_KEEP_ALIVE=${options.ollamaKeepAlive || '1h'}`,
                '-e',`OLLAMA_CONTEXT_LENGTH=${contextLength}`,
                '-e',`OLLAMA_NUM_CTX=${contextLength}`,
                ...cpuEnv,
                ...gpuFlags,
                ...cpuFlag,
                ...memFlag,
                ...shmFlag,
                'ollama/ollama:latest'
              );

              await docker(args);

              // Wait until ready (new container only)
              let timeoutMs = 600000; // 10 minutes to allow large model loads
              const start = Date.now();
              if (hostPublish) {
                console.log(chalk.blue('[INFO] ') + 'Waiting for Ollama to be ready on host :11434...');
                while (Date.now() - start < timeoutMs) {
                  try { await execCmd('bash',['-lc','curl -fsS http://localhost:11434/api/tags >/dev/null']); break; } catch(_) {}
                  await new Promise(r=>setTimeout(r,2000));
                }
              } else {
                console.log(chalk.blue('[INFO] ') + 'Waiting for Ollama container to be ready...');
                while (Date.now() - start < timeoutMs) {
                  try { await docker(['exec','raworc_ollama','ollama','list'], { silent: true }); break; } catch(_) {}
                  await new Promise(r=>setTimeout(r,2000));
                }
              }
              if (Date.now() - start >= timeoutMs) {
                throw new Error('Ollama did not become ready in time');
              }
              console.log(chalk.green('[SUCCESS] ') + 'Ollama is ready');

              // Ensure model available (best-effort)
              console.log(chalk.blue('[INFO] ') + `Pulling ${options.ollamaModel} model (if needed)...`);
              try { await docker(['exec','raworc_ollama','ollama','pull', options.ollamaModel], { silent: true }); console.log(chalk.green('[SUCCESS] ') + `${options.ollamaModel} model available`);} catch(_) { console.log(chalk.yellow('[WARNING] ') + `Failed to pull ${options.ollamaModel}. You may need to pull manually.`); }
              console.log();
              break;
            }

            case 'server': {
              console.log(chalk.blue('[INFO] ') + 'Ensuring API server is running...');
              if (await containerRunning('raworc_server')) { console.log(chalk.green('[SUCCESS] ') + 'Server already running'); console.log(); break; }
              if (await containerExists('raworc_server')) {
                await docker(['start','raworc_server']);
                console.log(chalk.green('[SUCCESS] ') + 'Server started');
                console.log();
                break;
              }
              const args = ['run','-d',
                '--name','raworc_server',
                '--network','raworc_network',
                '-p', `${String(options.serverApiPort || '9000')}:9000`,
                '-v', 'raworc_logs:/app/logs',
                '-e',`DATABASE_URL=${options.serverDatabaseUrl || 'mysql://raworc:raworc@raworc_mysql:3306/raworc'}`,
                '-e',`JWT_SECRET=${options.serverJwtSecret || process.env.JWT_SECRET || 'development-secret-key'}`,
                '-e',`RUST_LOG=${options.serverRustLog || 'info'}`,
                ...(options.serverRaworcHost ? ['-e', `RAWORC_HOST=${options.serverRaworcHost}`] : []),
                ...(options.serverRaworcPort ? ['-e', `RAWORC_PORT=${options.serverRaworcPort}`] : []),
                SERVER_IMAGE
              ];
              await docker(args);
              console.log(chalk.green('[SUCCESS] ') + 'API server container started');
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
                const res = await docker(['ps','-q','--filter','name=raworc_ollama'], { silent: true });
                const hasOllama = !!res.stdout.trim();
                if (!OLLAMA_HOST) OLLAMA_HOST = hasOllama ? 'http://raworc_ollama:11434' : 'http://host.docker.internal:11434';
              } catch (_) {
                if (!OLLAMA_HOST) OLLAMA_HOST = 'http://host.docker.internal:11434';
              }

              const agentImage = options.controllerAgentImage || AGENT_IMAGE;
              const controllerDbUrl = options.controllerDatabaseUrl || 'mysql://raworc:raworc@raworc_mysql:3306/raworc';
              const controllerJwt = options.controllerJwtSecret || process.env.JWT_SECRET || 'development-secret-key';
              const controllerRustLog = options.controllerRustLog || 'info';
              const model = options.controllerOllamaModel || options.ollamaModel;
              const args = ['run','-d',
                '--name','raworc_controller',
                '--network','raworc_network',
                '-v','/var/run/docker.sock:/var/run/docker.sock',
                '-e',`DATABASE_URL=${controllerDbUrl}`,
                '-e',`JWT_SECRET=${controllerJwt}`,
                '-e',`OLLAMA_HOST=${OLLAMA_HOST}`,
                '-e',`OLLAMA_MODEL=${model}`,
                ...(process.env.RAWORC_HOST_NAME ? ['-e', `RAWORC_HOST_NAME=${process.env.RAWORC_HOST_NAME}`] : []),
                ...(process.env.RAWORC_HOST_URL ? ['-e', `RAWORC_HOST_URL=${process.env.RAWORC_HOST_URL}`] : []),
                '-e',`AGENT_IMAGE=${agentImage}`,
                '-e',`AGENT_CPU_LIMIT=${options.controllerAgentCpuLimit || '0.5'}`,
                '-e',`AGENT_MEMORY_LIMIT=${options.controllerAgentMemoryLimit || '536870912'}`,
                '-e',`AGENT_DISK_LIMIT=${options.controllerAgentDiskLimit || '1073741824'}`,
                '-e',`RUST_LOG=${controllerRustLog}`,
                CONTROLLER_IMAGE
              ];
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
                const desiredId = await imageId(OPERATOR_IMAGE);
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
                '-p','7000:7000',
                '-v','raworc_content_data:/content',
                ...(process.env.RAWORC_HOST_NAME ? ['-e', `RAWORC_HOST_NAME=${process.env.RAWORC_HOST_NAME}`] : []),
                ...(process.env.RAWORC_HOST_URL ? ['-e', `RAWORC_HOST_URL=${process.env.RAWORC_HOST_URL}`] : []),
                OPERATOR_IMAGE
              );
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
              args.push('--name','raworc_gateway','--network','raworc_network','-p','80:80', GATEWAY_IMAGE);
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
              console.log('  • Gateway: http://localhost/');
              console.log('  • Operator UI: http://localhost/');
              console.log('  • API via Gateway: http://localhost/api');
              console.log('  • Content: http://localhost/content');
            } else {
              const s = await docker(['ps','--filter','name=raworc_server','--format','{{.Names}}'], { silent: true });
              if (s.stdout.trim()) {
                console.log('  • API Server: http://localhost:9000');
                console.log('  • Public Content: http://localhost:8000');
              }
            }
          } catch(_) {}
          try {
            const m = await docker(['ps','--filter','name=raworc_mysql','--format','{{.Names}}'], { silent: true });
            if (m.stdout.trim()) {
              console.log('  • MySQL Port: 3307');
            }
          } catch(_) {}
          console.log();
          console.log(chalk.blue('[INFO] ') + 'Next steps:');
          console.log('  • Check logs: docker logs raworc_server -f');
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
