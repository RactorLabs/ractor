const { spawn } = require('child_process');
const path = require('path');

class DockerManager {
  constructor() {
    // Use published Docker images from DigitalOcean Container Registry
    // Registry namespace: registry.digitalocean.com/ractor
    this.images = {
      mysql: 'mysql:8.0',
      api: 'registry.digitalocean.com/ractor/ractor_api:latest',
      controller: 'registry.digitalocean.com/ractor/ractor_controller:latest',
      agent: 'registry.digitalocean.com/ractor/ractor_agent:latest',
      operator: 'registry.digitalocean.com/ractor/ractor_operator:latest',
      gateway: 'registry.digitalocean.com/ractor/ractor_gateway:latest',
      content: 'registry.digitalocean.com/ractor/ractor_content:latest',
      app_githex: 'registry.digitalocean.com/ractor/ractor_app_githex:latest',
      app_askrepo: 'registry.digitalocean.com/ractor/ractor_app_askrepo:latest'
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
    const serviceList = services.length > 0 ? services : ['mysql', 'ractor_api', 'ractor_operator', 'ractor_content', 'ractor_controller', 'ractor_gateway'];
    
    // Map service names to component names
    const componentMap = {
      'ractor_api': 'api',
      'ractor_controller': 'controller',
      'mysql': 'mysql',
      'ractor_operator': 'operator',
      'ractor_gateway': 'gateway',
      'ractor_content': 'content'
    };

    const components = serviceList.map(service => componentMap[service] || service);

    // Pull images if requested
    if (pullImages) {
      console.log('📦 Pulling latest images from registry...');
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
      await this.execDocker(['network', 'inspect', 'ractor_network'], { silent: true });
    } catch (error) {
      await this.execDocker(['network', 'create', 'ractor_network']);
    }

    // Create volumes if they don't exist
    for (const volume of ['mysql_data', 'ractor_content_data', 'ollama_data', 'ractor_api_data', 'ractor_operator_data', 'ractor_controller_data']) {
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
      await this.execDocker(['stop', `ractor_${component}`], { silent: true });
      await this.execDocker(['rm', `ractor_${component}`], { silent: true });
    } catch (error) {
      // Container doesn't exist, that's fine
    }

    switch (component) {
      case 'mysql':
        await this.execDocker([
          'run', '-d',
          '--name', 'mysql',
          '--network', 'ractor_network',
          '-p', '3307:3306',
          '-v', 'mysql_data:/var/lib/mysql',
          '-e', 'MYSQL_ROOT_PASSWORD=root',
          '-e', 'MYSQL_DATABASE=ractor',
          '-e', 'MYSQL_USER=ractor',
          '-e', 'MYSQL_PASSWORD=ractor',
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
          '--name', 'ractor_operator',
          '--network', 'ractor_network',
          '-v', 'ractor_content_data:/content',
          '-v', 'ractor_operator_data:/app/logs',
          ...(process.env.RACTOR_HOST_NAME ? ['-e', `RACTOR_HOST_NAME=${process.env.RACTOR_HOST_NAME}`] : []),
          ...(process.env.RACTOR_HOST_URL ? ['-e', `RACTOR_HOST_URL=${process.env.RACTOR_HOST_URL}`] : []),
          this.images.operator
        ]);
        console.log('🚀 ractor_operator started');
        break;
      case 'content':
        await this.execDocker([
          'run', '-d',
          '--name', 'ractor_content',
          '--network', 'ractor_network',
          '-v', 'ractor_content_data:/content',
          this.images.content || 'registry.digitalocean.com/ractor/ractor_content:latest'
        ]);
        console.log('🚀 ractor_content started');
        break;

      case 'gateway':
        await this.execDocker([
          'run', '-d',
          '--name', 'ractor_gateway',
          '--network', 'ractor_network',
          '-p', '80:80',
          this.images.gateway
        ]);
        console.log('🚀 ractor_gateway started (port 80)');
        break;

      case 'api':
        await this.execDocker([
          'run', '-d',
          '--name', 'ractor_api',
          '--network', 'ractor_network',
          '-p', '9000:9000',
          '-v', 'ractor_api_data:/app/logs',
          '-e', 'DATABASE_URL=mysql://ractor:ractor@mysql:3306/ractor',
          '-e', 'JWT_SECRET=development-secret-key',
          '-e', 'RUST_LOG=info',
          this.images.api
        ]);
        break;

      case 'controller':
        await this.execDocker([
          'run', '-d',
          '--name', 'ractor_controller',
          '--network', 'ractor_network',
          '-v', '/var/run/docker.sock:/var/run/docker.sock',
          '-v', 'ractor_controller_data:/app/logs',
          '-e', 'DATABASE_URL=mysql://ractor:ractor@mysql:3306/ractor',
          '-e', 'JWT_SECRET=development-secret-key',
          ...(process.env.OLLAMA_HOST ? ['-e', `OLLAMA_HOST=${process.env.OLLAMA_HOST}`] : []),
          ...(process.env.RACTOR_HOST_NAME ? ['-e', `RACTOR_HOST_NAME=${process.env.RACTOR_HOST_NAME}`] : []),
          ...(process.env.RACTOR_HOST_URL ? ['-e', `RACTOR_HOST_URL=${process.env.RACTOR_HOST_URL}`] : []),
          '-e', `AGENT_IMAGE=${this.images.agent}`,
          '-e', 'AGENT_CPU_LIMIT=0.5',
          '-e', 'AGENT_MEMORY_LIMIT=536870912',
          '-e', 'AGENT_DISK_LIMIT=1073741824',
          '-e', 'RUST_LOG=info',
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
    const serviceList = services.length > 0 ? services : ['ractor_gateway', 'ractor_controller', 'ractor_operator', 'ractor_content', 'ractor_api'];
    
    // Map service names to component names
    const componentMap = {
      'ractor_api': 'api',
      'ractor_controller': 'controller',
      'mysql': 'mysql',
      'ractor_operator': 'operator',
      'ractor_gateway': 'gateway',
      'ractor_content': 'content'
    };

    const components = serviceList.map(service => componentMap[service] || service);

    // Stop in reverse order
    for (const component of components.reverse()) {
      try {
        await this.execDocker(['stop', `ractor_${component}`], { silent: true });
        await this.execDocker(['rm', `ractor_${component}`], { silent: true });
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
      const result = await this.execDocker(['ps', '--filter', 'name=ractor_', '--format', 'table {{.Names}}\\t{{.Status}}\\t{{.Ports}}'], { silent: true });
      return result.stdout;
    } catch (error) {
      return null;
    }
  }

  // Pull latest images from registry
  async pull(version = 'latest') {
    // Create version-specific image names
    const versionedImages = {};
    for (const [component, image] of Object.entries(this.images)) {
      if (component !== 'mysql') {
        // For ractor images, use the specified version tag
        if (image.startsWith('registry.digitalocean.com/ractor/') || image.startsWith('ractor/')) {
          const [repo] = image.split(':');
          versionedImages[component] = `${repo}:${version}`;
        } else {
          // For non-ractor images, use original (like python:3.11-slim)
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
      const result = await this.execDocker(['ps', '-a', '-q', '--filter', 'name=ractor_agent_'], { silent: true });
      
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
    // For published CLI, we can pull from the configured registry
    return true;
  }
}

module.exports = new DockerManager();
