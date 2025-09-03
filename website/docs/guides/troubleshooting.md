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

## Session containers

```bash
# List running session containers
docker ps --filter "name=raworc_session_"

# Clean restart
raworc start --restart

# Clean up all sessions
raworc clean --all

# Manual cleanup of specific session
docker rm -f raworc_session_{session-name}

# Check session container logs
docker logs raworc_session_{session-name} --tail 50

# Check Host logs inside session
docker exec raworc_session_{session-name} ls -la /session/logs/
docker exec raworc_session_{session-name} cat /session/logs/host_{timestamp}_stderr.log

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

