# Ecoaccounts Indexer Dashboard

A React-based dashboard for monitoring and controlling the Ecoaccounts Indexer.

## Features

- Real-time indexer status monitoring
- Pause/Resume indexer controls
- Reindex with custom block ranges and strategies
- Modern, responsive UI

## Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev
```

The development server runs on `http://localhost:5173`. 

Configure the API connection in `.env`:

```env
VITE_API_URL=http://localhost:3000
VITE_API_KEY=your-api-key
```

## Production Build

```bash
# Build for production
npm run build
```

This creates a `dist/` folder with the production build.

## Serving from the Indexer

The dashboard can be served directly from the Rust indexer on the same port:

1. Build the dashboard: `npm run build`
2. The indexer will automatically detect `dashboard/dist` and serve it at `/dashboard`
3. Access the dashboard at `http://localhost:3000/dashboard`

You can also set a custom path with the `DASHBOARD_PATH` environment variable:

```bash
DASHBOARD_PATH=/path/to/dashboard/dist cargo run
```

When served from the indexer:
- The dashboard is available at `/dashboard`
- The API endpoints are available at `/api/status`, `/api/pause`, `/api/resume`, `/api/reindex`
- Legacy API endpoints (`/status`, `/pause`, etc.) are still available for backward compatibility
