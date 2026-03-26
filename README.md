# ai-gateway

## Database

```bash
sudo -u postgres psql
CREATE USER aigateway WITH PASSWORD 'aigateway';
CREATE DATABASE aigatewaydb OWNER aigateway;
```

## Errors

```bash
cargo check 2>&1 | tee build_error.log.txt
```
