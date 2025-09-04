const { spawn } = require('child_process');
const path = require('path');

class DockerManager {
  constructor() {
    // Use published Docker images from Docker Hub
    this.images = {
      mysql: 'mysql:8.0',
      server: 'raworc/raworc_server:latest',
      operator: 'raworc/raworc_operator:latest',
      agent: 'raworc/raworc_agent:latest'
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
    // Default to all services if none specified
    const serviceList = services.length > 0 ? services : ['raworc_mysql', 'raworc_server', 'raworc_operator'];
    
    // Map service names to component names
    const componentMap = {
      'raworc_server': 'server',
      'raworc_operator': 'operator',
      'raworc_mysql': 'mysql'
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
            console.warn(`   The operator will attempt to pull this image when needed.`);
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
    for (const volume of ['raworc_mysql_data']) {
      try {
        await this.execDocker(['volume', 'inspect', volume], { silent: true });
      } catch (error) {
        await this.execDocker(['volume', 'create', volume]);
      }
    }

    // Start services in order: mysql, server, operator
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
          '--name', 'raworc_mysql',
          '--network', 'raworc_network',
          '-p', '3307:3306',
          '-v', 'raworc_mysql_data:/var/lib/mysql',
          '-e', 'MYSQL_ROOT_PASSWORD=root',
          '-e', 'MYSQL_DATABASE=raworc',
          '-e', 'MYSQL_USER=raworc',
          '-e', 'MYSQL_PASSWORD=raworc',
          '--health-cmd', 'mysqladmin ping -h localhost -u root -proot',
          '--health-interval', '10s',
          '--health-timeout', '5s',
          '--health-retries', '5',
          this.images.mysql,
          '--default-authentication-plugin=mysql_native_password',
          '--collation-server=utf8mb4_unicode_ci',
          '--character-set-server=utf8mb4'
        ]);
        
        // Wait for MySQL to be healthy
        await this.waitForMysql();
        break;

      case 'server':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_server',
          '--network', 'raworc_network',
          '-p', '9000:9000',
          '-v', `${process.cwd()}/logs:/app/logs`,
          '-e', 'DATABASE_URL=mysql://raworc:raworc@raworc_mysql:3306/raworc',
          '-e', 'JWT_SECRET=development-secret-key',
          '-e', 'RUST_LOG=info',
          this.images.server
        ]);
        break;

      case 'operator':
        await this.execDocker([
          'run', '-d',
          '--name', 'raworc_operator',
          '--network', 'raworc_network',
          '-v', '/var/run/docker.sock:/var/run/docker.sock',
          '-e', 'DATABASE_URL=mysql://raworc:raworc@raworc_mysql:3306/raworc',
          '-e', 'JWT_SECRET=development-secret-key',
          '-e', `ANTHROPIC_API_KEY=${process.env.ANTHROPIC_API_KEY}`,
          '-e', `AGENT_IMAGE=${this.images.agent}`,
          '-e', 'AGENT_CPU_LIMIT=0.5',
          '-e', 'AGENT_MEMORY_LIMIT=536870912',
          '-e', 'AGENT_DISK_LIMIT=1073741824',
          '-e', 'RUST_LOG=info',
          this.images.operator
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
        await this.execDocker(['exec', 'raworc_mysql', 'mysqladmin', 'ping', '-h', 'localhost', '-u', 'root', '-proot'], { silent: true });
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
    const serviceList = services.length > 0 ? services : ['raworc_operator', 'raworc_server', 'raworc_mysql'];
    
    // Map service names to component names
    const componentMap = {
      'raworc_server': 'server',
      'raworc_operator': 'operator',
      'raworc_mysql': 'mysql'
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
          console.warn(`   The operator will attempt to pull this image when needed.`);
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