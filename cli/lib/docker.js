const { spawn } = require('child_process');
const path = require('path');

class DockerManager {
  constructor() {
    // Use published Docker images from Docker Hub
    this.images = {
      mysql: 'mysql:8.0',
      api: 'raworc/raworc_api:latest',
      controller: 'raworc/raworc_controller:latest',
      agent: 'raworc/raworc_agent:latest',
      operator: 'raworc/raworc_operator:latest',
      gateway: 'raworc/raworc_gateway:latest',
      content: 'raworc/raworc_content:latest'
    };
  }

  // Execute Docker command
  async execDocker(args, options = {}) {
    return new Promise((resolve, reject) => {
      const docker = spawn('docker', args, {
        stdio: options.silent ? 'pipe' : 'inherit',
        ...options
      });

      let stdout = '';
      let stderr = '';

      if (options.silent) {
        docker.stdout.on('data', (data) => {
          stdout += data.toString();
        });

        docker.stderr.on('data', (data) => {
          stderr += data.toString();
        });
      }

      docker.on('exit', (code) => {
        if (code === 0) {
          resolve({ code, stdout, stderr });
        } else {
          reject(new Error(`Docker command failed with code ${code}: ${stderr || 'Unknown error'}`));
        }
      });

      docker.on('error', (error) => {
        reject(new Error(`Failed to execute docker command: ${error.message}`));
      });
    });
  }

  // Start services using direct Docker commands with published images
  async start(services = [], pullImages = false) {
    // Default to full stack if none specified
    const serviceList = services.length > 0 ? services : ['mysql', 'raworc_api', 'raworc_operator', 'raworc_content', 'raworc_controller', 'raworc_gateway'];
    
    // Map service names to component names
    const componentMap = {
      'raworc_api': 'api',
      'raworc_controller': 'controller',
      'mysql': 'mysql',
      'raworc_operator': 'operator',
      'raworc_gateway': 'gateway',
      'raworc_content': 'content'
    };

    const components = serviceList.map(service => componentMap[service] || service);

    // Pull images if requested
    if (pullImages) {
      console.log('📦 Pulling latest images from Docker Hub...');
      for (const component of components) {
        if (this.images[component]) {
          try {
            console.log(`📦 Pulling ${this.images[component]}...`);
            await this.execDocker(['pull', this.images[component]]);
            console.log(`✅ Successfully pulled ${this.images[component]}`);
          } catch (error) {
            console.warn(`⚠️  Warning: Failed to pull ${this.images[component]}: ${error.message}`);
            console.warn(`   The controller will attempt to pull this image when needed.`);
          }
        }
      }
    }

    // Create network if it doesn't exist
    try {
      await this.execDocker(['network', 'inspect', 'raworc_network'], { silent: true });
    } catch (error) {
      await this.execDocker(['network', 'create', 'raworc_network']);
    }

    // Create volumes if they don't exist
    for (const volume of ['mysql_data', 'raworc_content_data', 'raworc_gpt_data', 'raworc_gpt_logs', 'raworc_api_data', 'raworc_operator_data', 'raworc_controller_data']) {
      try {
        await this.execDocker(['volume', 'inspect', volume], { silent: true });
      } catch (error) {
        await this.execDocker(['volume', 'create', volume]);
      }
    }

    // Start services in order
    for (const component of components) {
      await this.startService(component);
    }
  }

  // Start individual service
  async startService(component) {
    // Stop and remove existing container
    try {
      await this.execDocker(['stop', `raworc_${component}`], { silent: true });
      await this.execDocker(['rm', `raworc_${component}`], { silent: true });
    } catch (error) {
      // Container doesn't exist, that's fine
    }

    switch (component) {
      case 'mysql':
        await this.execDocker([
          'run', '-d',
          '--name', 'mysql',
          '--network', 'raworc_network',
          '-p', '3307:3306',
          '-v', 'mysql_data:/var/lib/mysql',
          '-e', 'MYSQL_ROOT_PASSWORD=root',
          '-e', 'MYSQL_DATABASE=raworc',
          '-e', 'MYSQL_USER=raworc',
          '-e', 'MYSQL_PASSWORD=raworc',
          '--health-cmd', 'mysqladmin ping -h localhost -u root -proot',
          '--health-interval', '10s',
          '--health-timeout', '5s',
          '--health-retries', '5',
          this.images.mysql,
          // Persist logs into data volume
          '--log-error=/var/lib/mysql/mysql-error.log',
          '--slow_query_log=ON',
          '--long_query_time=2',
          '--slow_query_log_file=/var/lib/mysql/mysql-slow.log',
          '--default-authentication-plugin=mysql_native_password',
          '--collation-server=utf8mb4_unicode_ci',
          '--character-set-server=utf8mb4'
        ]);
        
        // Wait for MySQL to be healthy
        await this.waitForMysql();
        break;

      case 'operator':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_operator',
          '--network', 'raworc_network',
          '-v', 'raworc_content_data:/content',
          '-v', 'raworc_operator_data:/app/logs',
          ...(process.env.RAWORC_HOST_NAME ? ['-e', `RAWORC_HOST_NAME=${process.env.RAWORC_HOST_NAME}`] : []),
          ...(process.env.RAWORC_HOST_URL ? ['-e', `RAWORC_HOST_URL=${process.env.RAWORC_HOST_URL}`] : []),
          this.images.operator
        ]);
        console.log('🚀 raworc_operator started');
        break;
      case 'content':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_content',
          '--network', 'raworc_network',
          '-v', 'raworc_content_data:/content',
          this.images.content || 'raworc/raworc_content:latest'
        ]);
        console.log('🚀 raworc_content started');
        break;

      case 'gateway':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_gateway',
          '--network', 'raworc_network',
          '-p', '80:80',
          this.images.gateway
        ]);
        console.log('🚀 raworc_gateway started (port 80)');
        break;

      case 'api':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_api',
          '--network', 'raworc_network',
          '-p', '9000:9000',
          '-v', 'raworc_api_data:/app/logs',
          '-e', 'DATABASE_URL=mysql://raworc:raworc@mysql:3306/raworc',
          '-e', 'JWT_SECRET=development-secret-key',
          '-e', 'RUST_LOG=debug',
          this.images.api
        ]);
        break;

      case 'controller':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_controller',
          '--network', 'raworc_network',
          '-v', '/var/run/docker.sock:/var/run/docker.sock',
          '-v', 'raworc_controller_data:/app/logs',
          '-e', 'DATABASE_URL=mysql://raworc:raworc@mysql:3306/raworc',
          '-e', 'JWT_SECRET=development-secret-key',
          ...(process.env.RAWORC_GPT_URL ? ['-e', `RAWORC_GPT_URL=${process.env.RAWORC_GPT_URL}`] : []),
          ...(process.env.RAWORC_HOST_NAME ? ['-e', `RAWORC_HOST_NAME=${process.env.RAWORC_HOST_NAME}`] : []),
          ...(process.env.RAWORC_HOST_URL ? ['-e', `RAWORC_HOST_URL=${process.env.RAWORC_HOST_URL}`] : []),
          '-e', `AGENT_IMAGE=${this.images.agent}`,
          '-e', 'AGENT_CPU_LIMIT=0.5',
          '-e', 'AGENT_MEMORY_LIMIT=536870912',
          '-e', 'AGENT_DISK_LIMIT=1073741824',
          '-e', 'RUST_LOG=debug',
          this.images.controller
        ]);
        break;

      default:
        throw new Error(`Unknown component: ${component}`);
    }
  }

  // Wait for MySQL to be healthy
  async waitForMysql() {
    console.log('⏳ Waiting for MySQL to be ready...');
    for (let i = 0; i < 30; i++) {
      try {
        await this.execDocker(['exec', 'mysql', 'mysqladmin', 'ping', '-h', 'localhost', '-u', 'root', '-proot'], { silent: true });
        console.log('✅ MySQL is ready');
        return;
      } catch (error) {
        await new Promise(resolve => setTimeout(resolve, 2000));
      }
    }
    throw new Error('MySQL failed to become healthy');
  }

  // Stop services
  async stop(services = [], cleanup = false) {
    // Default to stopping gateway, controller, operator and api
    const serviceList = services.length > 0 ? services : ['raworc_gateway', 'raworc_controller', 'raworc_operator', 'raworc_content', 'raworc_api'];
    
    // Map service names to component names
    const componentMap = {
      'raworc_api': 'api',
      'raworc_controller': 'controller',
      'mysql': 'mysql',
      'raworc_operator': 'operator',
      'raworc_gateway': 'gateway',
      'raworc_content': 'content'
    };

    const components = serviceList.map(service => componentMap[service] || service);

    // Stop in reverse order
    for (const component of components.reverse()) {
      try {
        await this.execDocker(['stop', `raworc_${component}`], { silent: true });
        await this.execDocker(['rm', `raworc_${component}`], { silent: true });
      } catch (error) {
        // Container might not exist
      }
    }

    // Clean up agent containers if requested
    if (cleanup) {
      await this.cleanupContainers();
    }
  }

  // Get service status
  async status() {
    try {
      const result = await this.execDocker(['ps', '--filter', 'name=raworc_', '--format', 'table {{.Names}}\\t{{.Status}}\\t{{.Ports}}'], { silent: true });
      return result.stdout;
    } catch (error) {
      return null;
    }
  }

  // Pull latest images from Docker Hub
  async pull(version = 'latest') {
    // Create version-specific image names
    const versionedImages = {};
    for (const [component, image] of Object.entries(this.images)) {
      if (component !== 'mysql') {
        // For raworc images, use the specified version tag
        if (image.startsWith('raworc/')) {
          const [repo] = image.split(':');
          versionedImages[component] = `${repo}:${version}`;
        } else {
          // For non-raworc images, use original (like python:3.11-slim)
          versionedImages[component] = image;
        }
      }
    }

    for (const [component, image] of Object.entries(versionedImages)) {
      console.log(`📦 Pulling ${image}...`);
      try {
        await this.execDocker(['pull', image]);
        console.log(`✅ Successfully pulled ${image}`);
      } catch (error) {
        console.warn(`⚠️  Warning: Failed to pull ${image}: ${error.message}`);
        if (component === 'agent') {
          console.warn(`   The controller will attempt to pull this image when needed.`);
        }
      }
    }
  }

  // Check if Docker is available
  async checkDocker() {
    try {
      await this.execDocker(['--version'], { silent: true });
      return true;
    } catch (error) {
      return false;
    }
  }

  // Check if Docker Compose is available (not needed for CLI)
  async checkDockerCompose() {
    // CLI doesn't use Docker Compose, but keep for compatibility
    return this.checkDocker();
  }

  // Clean up agent containers
  async cleanupContainers() {
    try {
      const result = await this.execDocker(['ps', '-a', '-q', '--filter', 'name=raworc_agent_'], { silent: true });
      
      if (result.stdout.trim()) {
        const containerIds = result.stdout.trim().split('\n').filter(id => id);
        if (containerIds.length > 0) {
          await this.execDocker(['rm', '-f', ...containerIds]);
          return containerIds.length;
        }
      }
      
      return 0;
    } catch (error) {
      throw new Error(`Failed to cleanup containers: ${error.message}`);
    }
  }

  // Check if required Docker images are available (either locally or can be pulled)
  async checkImages() {
    // For published CLI, we can always pull from Docker Hub
    return true;
  }
}

module.exports = new DockerManager();
