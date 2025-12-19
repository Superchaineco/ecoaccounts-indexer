# superaccounts-indexer

Blockchain indexer for SuperAccounts with built-in control API.

## API

The indexer exposes an HTTP API for monitoring and control.

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `API_PORT` | `3000` | API server port |
| `API_KEY` | `changeme` | Authentication key |

### Authentication

All endpoints require the `X-API-Key` header:

```bash
curl -H "X-API-Key: your-api-key" http://localhost:3000/status
```

### Endpoints

#### `GET /status`

Returns the current indexer status.

**Response:**
```json
{
  "status": "running",
  "last_block": 125901332,
  "head": 125902000,
  "behind": 668
}
```

Status values: `running`, `paused`, `reindexing`

---

#### `POST /pause`

Pauses the indexer. It will stop processing new blocks until resumed.

**Response:**
```json
{ "ok": true, "msg": "paused" }
```

---

#### `POST /resume`

Resumes a paused indexer.

**Response:**
```json
{ "ok": true, "msg": "resumed" }
```

---

#### `POST /reindex`

Triggers a re-indexation. All fields are optional.

**Request body:**
```json
{
  "from": 125000000,
  "to": 126000000,
  "strategy": "super_account_created"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `from` | `u64?` | Start block. Default: strategy's original `from_block` |
| `to` | `u64?` | End block. Default: last indexed block |
| `strategy` | `string?` | Strategy name to reindex. Default: all strategies |

**Examples:**

```bash
# Reindex everything from origin
curl -X POST -H "X-API-Key: key" http://localhost:3000/reindex

# Reindex specific strategy
curl -X POST -H "X-API-Key: key" -H "Content-Type: application/json" \
  -d '{"strategy": "badges_minted"}' \
  http://localhost:3000/reindex

# Reindex specific range
curl -X POST -H "X-API-Key: key" -H "Content-Type: application/json" \
  -d '{"from": 125000000, "to": 126000000}' \
  http://localhost:3000/reindex
```

**Response:**
```json
{ "ok": true, "msg": "reindexing from origin" }
```

