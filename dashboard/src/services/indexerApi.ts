import axios from 'axios';
import type { IndexerStatus, ApiResponse, ReindexRequest } from '../types/api';

const getApiBase = () => {
  if (window.location.pathname.startsWith('/dashboard')) {
    return window.location.origin + '/api';
  }
  return 'http://localhost:3000/api';
};

const API_BASE = getApiBase();
const API_KEY = import.meta.env.VITE_API_KEY || 'your-api-key';

const apiClient = axios.create({
  baseURL: API_BASE,
  headers: {
    'X-API-Key': API_KEY,
    'Content-Type': 'application/json',
  },
});

export const indexerApi = {
  getStatus: async (): Promise<IndexerStatus> => {
    const response = await apiClient.get<IndexerStatus>('/status');
    return response.data;
  },

  pause: async (): Promise<ApiResponse> => {
    const response = await apiClient.post<ApiResponse>('/pause');
    return response.data;
  },

  resume: async (): Promise<ApiResponse> => {
    const response = await apiClient.post<ApiResponse>('/resume');
    return response.data;
  },

  reindex: async (params: ReindexRequest): Promise<ApiResponse> => {
    const response = await apiClient.post<ApiResponse>('/reindex', params);
    return response.data;
  },
};