export interface IndexerStatus {
  status: 'running' | 'paused' | 'reindexing';
  last_block: number;
  head: number;
  behind: number;
  index?: {
    from: number;
    to: number;
    current: number;
    strategy?: string;
    is_reindex: boolean;
  };
}

export interface ApiResponse {
  ok: boolean;
  msg: string;
}

export interface ReindexRequest {
  from?: number;
  to?: number;
  strategy?: string;
}