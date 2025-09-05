---
sidebar_position: 5
title: Troubleshooting
---

# Troubleshooting

## Services won't start

```bash
# Check if ports are in use
sudo lsof -i :9000
sudo lsof -i :3307

# View service logs
docker logs raworc_server
docker logs raworc_operator
docker logs raworc_mysql

# Restart services
raworc start --restart

# Complete reset
raworc reset --yes
```

## Database connection issues

```bash
# Check database connectivity
docker exec raworc_mysql mysql -u raworc -praworc -e "SELECT 1"

# View database logs
docker logs raworc_mysql

# Check migrations
docker logs raworc_server | grep migration
```

## Agent containers

```bash
# List running agent containers
docker ps --filter "name=raworc_agent_"

# Clean restart
raworc start --restart

# Clean up all agents
raworc clean

# Manual cleanup of specific agent container
docker rm -f raworc_agent_{agent-name}

# Check agent container logs
docker logs raworc_agent_{agent-name} --tail 50

# Complete system reset
raworc reset --yes
```

## API Connection Issues

```bash
# Check if services are running
docker ps --filter "name=raworc_"

# Test API connectivity
raworc api version

# Check authentication
raworc auth
raworc login --user admin --pass admin
raworc auth --token <jwt-token-from-login>
```
