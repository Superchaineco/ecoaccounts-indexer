import React, { useState } from 'react';
import {
  Card,
  CardContent,
  Typography,
  Button,
  Box,
  TextField,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Alert,
  Snackbar,
} from '@mui/material';
import { PlayArrow, Pause, Refresh } from '@mui/icons-material';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { indexerApi } from '../services/indexerApi';
import type { ReindexRequest } from '../types/api';

interface ControlPanelProps {
  status: string;
  disabled?: boolean;
}

export const ControlPanel: React.FC<ControlPanelProps> = ({ status, disabled }) => {
  const [reindexDialogOpen, setReindexDialogOpen] = useState(false);
  const [reindexParams, setReindexParams] = useState<ReindexRequest>({});
  const [snackbar, setSnackbar] = useState<{ open: boolean; message: string; severity: 'success' | 'error' }>({
    open: false,
    message: '',
    severity: 'success'
  });

  const queryClient = useQueryClient();

  const pauseMutation = useMutation({
    mutationFn: indexerApi.pause,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      setSnackbar({ open: true, message: 'Indexer paused', severity: 'success' });
    },
    onError: () => {
      setSnackbar({ open: true, message: 'Failed to pause indexer', severity: 'error' });
    }
  });

  const resumeMutation = useMutation({
    mutationFn: indexerApi.resume,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      setSnackbar({ open: true, message: 'Indexer resumed', severity: 'success' });
    },
    onError: () => {
      setSnackbar({ open: true, message: 'Failed to resume indexer', severity: 'error' });
    }
  });

  const reindexMutation = useMutation({
    mutationFn: indexerApi.reindex,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['status'] });
      setSnackbar({ open: true, message: 'Reindex started', severity: 'success' });
      setReindexDialogOpen(false);
      setReindexParams({});
    },
    onError: () => {
      setSnackbar({ open: true, message: 'Failed to start reindex', severity: 'error' });
    }
  });

  const handlePause = () => {
    pauseMutation.mutate();
  };

  const handleResume = () => {
    resumeMutation.mutate();
  };

  const handleReindex = () => {
    const params: ReindexRequest = {};
    if (reindexParams.from) params.from = Number(reindexParams.from);
    if (reindexParams.to) params.to = Number(reindexParams.to);
    if (reindexParams.strategy) params.strategy = reindexParams.strategy;
    
    reindexMutation.mutate(params);
  };

  return (
    <>
      <Card>
        <CardContent>
          <Typography variant="h6" component="h2" mb={2}>
            Control Panel
          </Typography>
          
          <Box display="flex" gap={2} flexWrap="wrap">
            {status === 'running' || status === 'reindexing' ? (
              <Button
                variant="contained"
                color="warning"
                startIcon={<Pause />}
                onClick={handlePause}
                disabled={disabled || pauseMutation.isPending}
              >
                Pause
              </Button>
            ) : (
              <Button
                variant="contained"
                color="success"
                startIcon={<PlayArrow />}
                onClick={handleResume}
                disabled={disabled || resumeMutation.isPending}
              >
                Resume
              </Button>
            )}
            
            <Button
              variant="outlined"
              color="primary"
              startIcon={<Refresh />}
              onClick={() => setReindexDialogOpen(true)}
              disabled={disabled}
            >
              Reindex
            </Button>
          </Box>
        </CardContent>
      </Card>

      <Dialog open={reindexDialogOpen} onClose={() => setReindexDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Reindex Configuration</DialogTitle>
        <DialogContent>
          <Box display="flex" flexDirection="column" gap={2} mt={1}>
            <TextField
              label="From Block (optional)"
              type="number"
              value={reindexParams.from || ''}
              onChange={(e) => setReindexParams(prev => ({ ...prev, from: e.target.value ? Number(e.target.value) : undefined }))}
              helperText="Leave empty to use strategy default"
            />
            <TextField
              label="To Block (optional)"
              type="number"
              value={reindexParams.to || ''}
              onChange={(e) => setReindexParams(prev => ({ ...prev, to: e.target.value ? Number(e.target.value) : undefined }))}
              helperText="Leave empty to use current head block"
            />
            <TextField
              label="Strategy (optional)"
              value={reindexParams.strategy || ''}
              onChange={(e) => setReindexParams(prev => ({ ...prev, strategy: e.target.value || undefined }))}
              helperText="Specific indexing strategy to use"
            />
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setReindexDialogOpen(false)}>Cancel</Button>
          <Button 
            onClick={handleReindex} 
            variant="contained"
            disabled={reindexMutation.isPending}
          >
            Start Reindex
          </Button>
        </DialogActions>
      </Dialog>

      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={() => setSnackbar(prev => ({ ...prev, open: false }))}
      >
        <Alert 
          onClose={() => setSnackbar(prev => ({ ...prev, open: false }))} 
          severity={snackbar.severity}
          sx={{ width: '100%' }}
        >
          {snackbar.message}
        </Alert>
      </Snackbar>
    </>
  );
};