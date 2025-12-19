import { useState } from 'react';
import { QueryClient, QueryClientProvider, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { indexerApi } from './services/indexerApi';
import type { IndexerStatus, ReindexRequest } from './types/api';
import './App.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchInterval: 3000,
      staleTime: 1000,
    },
  },
});

// Icons as simple SVG components
const BlockIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M3 9h18M9 21V9" />
  </svg>
);

const HeadIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 2v20M2 12h20" />
    <circle cx="12" cy="12" r="4" />
  </svg>
);

const BehindIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M13 17l5-5-5-5M6 17l5-5-5-5" />
  </svg>
);

const PlayIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
    <path d="M8 5v14l11-7z" />
  </svg>
);

const PauseIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
    <path d="M6 4h4v16H6zM14 4h4v16h-4z" />
  </svg>
);

const RefreshIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M23 4v6h-6M1 20v-6h6" />
    <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
  </svg>
);

// Header Component
function Header({ status }: { status?: IndexerStatus }) {
  const statusClass = status?.status || 'running';
  
  return (
    <header className="header">
      <div className="header-content">
        <div className="logo-section">
          <div className="logo-icon">⚡</div>
          <span className="logo-text">Ecoaccounts Indexer</span>
        </div>
        {status && (
          <div className={`status-badge ${statusClass}`}>
            <span className="status-dot" />
            {status.status}
          </div>
        )}
      </div>
    </header>
  );
}

// Stat Card Component
function StatCard({ 
  icon, 
  label, 
  value, 
  subtitle,
  iconColor = 'blue'
}: { 
  icon: React.ReactNode; 
  label: string; 
  value: string | number; 
  subtitle?: string;
  iconColor?: 'blue' | 'green' | 'purple' | 'orange';
}) {
  return (
    <div className="stat-card">
      <div className="stat-header">
        <span className="stat-label">{label}</span>
        <div className={`card-icon ${iconColor}`}>{icon}</div>
      </div>
      <div className="stat-value">{typeof value === 'number' ? value.toLocaleString() : value}</div>
      {subtitle && <div className="stat-subtitle">{subtitle}</div>}
    </div>
  );
}

// Progress Card Component
function ProgressCard({ status }: { status: IndexerStatus }) {
  if (!status.index) return null;
  
  const { from, to, current, strategy, is_reindex } = status.index;
  
  // When from and to are both 0, the backend is still calculating the range
  const isCalculating = from === 0 && to === 0;
  const progress = !isCalculating && to > from ? ((current - from) / (to - from)) * 100 : 0;
  
  return (
    <div className="progress-section">
      <div className="progress-card">
        <div className="progress-header">
          <span className="progress-title">
            {is_reindex ? '🔄 Reindexing' : '📊 Indexing'} Progress
            {strategy && ` - ${strategy}`}
          </span>
          <span className="progress-percentage">
            {isCalculating ? 'Preparing...' : `${progress.toFixed(1)}%`}
          </span>
        </div>
        <div className="progress-bar-container">
          {isCalculating ? (
            <div className="progress-bar progress-bar-indeterminate" />
          ) : (
            <div className="progress-bar" style={{ width: `${Math.min(progress, 100)}%` }} />
          )}
        </div>
        <div className="progress-details">
          <span>Block {current.toLocaleString()}</span>
          <span>
            {isCalculating 
              ? 'Calculating block range...' 
              : `From ${from.toLocaleString()} to ${to.toLocaleString()}`
            }
          </span>
        </div>
      </div>
    </div>
  );
}

// Reindex Modal Component
function ReindexModal({ 
  isOpen, 
  onClose, 
  onSubmit 
}: { 
  isOpen: boolean; 
  onClose: () => void; 
  onSubmit: (params: ReindexRequest) => void;
}) {
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [strategy, setStrategy] = useState('');

  if (!isOpen) return null;

  const handleSubmit = () => {
    onSubmit({
      from: from ? parseInt(from) : undefined,
      to: to ? parseInt(to) : undefined,
      strategy: strategy || undefined,
    });
    onClose();
    setFrom('');
    setTo('');
    setStrategy('');
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <h2 className="modal-title">Start Reindex</h2>
          <button className="modal-close" onClick={onClose}>&times;</button>
        </div>
        <div className="modal-body">
          <div className="input-group">
            <label className="input-label">From Block (optional)</label>
            <input
              type="number"
              className="input"
              placeholder="Start block number"
              value={from}
              onChange={e => setFrom(e.target.value)}
            />
          </div>
          <div className="input-group">
            <label className="input-label">To Block (optional)</label>
            <input
              type="number"
              className="input"
              placeholder="End block number"
              value={to}
              onChange={e => setTo(e.target.value)}
            />
          </div>
          <div className="input-group">
            <label className="input-label">Strategy (optional)</label>
            <input
              type="text"
              className="input"
              placeholder="Strategy name"
              value={strategy}
              onChange={e => setStrategy(e.target.value)}
            />
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-outline" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={handleSubmit}>Start Reindex</button>
        </div>
      </div>
    </div>
  );
}

// Alert Component
function Alert({ 
  message, 
  type, 
  onClose 
}: { 
  message: string; 
  type: 'success' | 'error'; 
  onClose: () => void;
}) {
  return (
    <div className={`alert ${type}`} onClick={onClose}>
      {message}
    </div>
  );
}

// Control Panel Component
function ControlPanel({ status, disabled }: { status: string; disabled?: boolean }) {
  const queryClient = useQueryClient();
  const [showReindexModal, setShowReindexModal] = useState(false);
  const [alert, setAlert] = useState<{ message: string; type: 'success' | 'error' } | null>(null);

  const showAlert = (message: string, type: 'success' | 'error') => {
    setAlert({ message, type });
    setTimeout(() => setAlert(null), 3000);
  };

  const pauseMutation = useMutation({
    mutationFn: indexerApi.pause,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      showAlert('Indexer paused successfully', 'success');
    },
    onError: () => showAlert('Failed to pause indexer', 'error'),
  });

  const resumeMutation = useMutation({
    mutationFn: indexerApi.resume,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      showAlert('Indexer resumed successfully', 'success');
    },
    onError: () => showAlert('Failed to resume indexer', 'error'),
  });

  const reindexMutation = useMutation({
    mutationFn: indexerApi.reindex,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      showAlert('Reindex started successfully', 'success');
    },
    onError: () => showAlert('Failed to start reindex', 'error'),
  });

  const isPaused = status === 'paused';
  const isLoading = pauseMutation.isPending || resumeMutation.isPending || reindexMutation.isPending;

  return (
    <>
      <div className="card">
        <div className="card-header">
          <h3 className="card-title">
            <div className="card-icon purple">⚙️</div>
            Controls
          </h3>
        </div>
        <div className="btn-group">
          {isPaused ? (
            <button
              className="btn btn-success"
              onClick={() => resumeMutation.mutate()}
              disabled={disabled || isLoading}
            >
              <PlayIcon /> Resume
            </button>
          ) : (
            <button
              className="btn btn-warning"
              onClick={() => pauseMutation.mutate()}
              disabled={disabled || isLoading}
            >
              <PauseIcon /> Pause
            </button>
          )}
          <button
            className="btn btn-primary"
            onClick={() => setShowReindexModal(true)}
            disabled={disabled || isLoading}
          >
            <RefreshIcon /> Reindex
          </button>
        </div>
      </div>

      <ReindexModal
        isOpen={showReindexModal}
        onClose={() => setShowReindexModal(false)}
        onSubmit={params => reindexMutation.mutate(params)}
      />

      {alert && (
        <Alert
          message={alert.message}
          type={alert.type}
          onClose={() => setAlert(null)}
        />
      )}
    </>
  );
}

// Main Dashboard Component
function Dashboard() {
  const { data: status, isLoading, error } = useQuery({
    queryKey: ['status'],
    queryFn: indexerApi.getStatus,
  });

  if (isLoading) {
    return (
      <>
        <Header />
        <main className="main-content">
          <div className="loading-container">
            <div className="spinner" />
            <span className="loading-text">Connecting to indexer...</span>
          </div>
        </main>
      </>
    );
  }

  if (error) {
    return (
      <>
        <Header />
        <main className="main-content">
          <div className="error-container">
            <span className="error-icon">⚠️</span>
            <div>
              <strong>Connection Failed</strong>
              <p style={{ margin: '0.5rem 0 0', opacity: 0.8 }}>
                Unable to connect to the indexer API. Please check your connection and API configuration.
              </p>
            </div>
          </div>
        </main>
      </>
    );
  }

  if (!status) {
    return (
      <>
        <Header />
        <main className="main-content">
          <div className="error-container">
            <span className="error-icon">📭</span>
            <span>No status data available</span>
          </div>
        </main>
      </>
    );
  }

  return (
    <>
      <Header status={status} />
      <main className="main-content">
        <div className="stats-grid">
          <StatCard
            icon={<BlockIcon />}
            label="Current Block"
            value={status.last_block}
            subtitle="Last indexed block"
            iconColor="blue"
          />
          <StatCard
            icon={<HeadIcon />}
            label="Chain Head"
            value={status.head}
            subtitle="Latest block on chain"
            iconColor="green"
          />
          <StatCard
            icon={<BehindIcon />}
            label="Blocks Behind"
            value={status.behind}
            subtitle={status.behind === 0 ? 'Fully synced! ✓' : 'Catching up...'}
            iconColor={status.behind === 0 ? 'green' : 'orange'}
          />
        </div>

        <ProgressCard status={status} />

        <div className="controls-grid">
          <ControlPanel status={status.status} />
        </div>
      </main>
    </>
  );
}

// App Component
function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="dashboard-container">
        <Dashboard />
      </div>
    </QueryClientProvider>
  );
}

export default App;
