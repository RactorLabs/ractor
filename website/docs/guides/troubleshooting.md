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
docker compose logs raworc_server
docker compose logs raworc_operator
docker compose logs raworc_mysql

# Restart services
docker compose restart

# Complete reset
docker compose down && docker system prune -f && docker compose up -d
```

## Database connection issues

```bash
# Check database connectivity
docker exec raworc_mysql mysql -u raworc -praworc -e "SELECT 1"

# View database logs
docker compose logs raworc_mysql

# Check migrations
docker logs raworc_server | grep migration
```

## Session containers

```bash
# List running agent containers
docker ps --filter "name=raworc_session_"

# Clean restart (Community Edition)
docker compose restart

# Manual cleanup of specific session
docker rm -f raworc_session_{session-id}

# Check session container logs
docker logs raworc_session_{session-id} --tail 50

# Check agent logs inside session
docker exec raworc_session_{session-id} ls -la /session/logs/
docker exec raworc_session_{session-id} cat /session/logs/{agent}_{timestamp}_stderr.log

# Complete system reset
docker compose down -v && docker system prune -f && docker compose up -d
```

## API Connection Issues

```bash
# Check if services are running
docker compose ps

# Test API connectivity
raworc api health

# Check authentication
raworc auth login --user admin --pass admin
```

