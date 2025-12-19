import React from 'react';
import {
  Card,
  CardContent,
  Typography,
  Box,
  Chip,
  LinearProgress,
} from '@mui/material';
import type { IndexerStatus } from '../types/api';

interface StatusCardProps {
  status: IndexerStatus;
}

const getStatusColor = (status: string) => {
  switch (status) {
    case 'running':
      return 'success';
    case 'paused':
      return 'warning';
    case 'reindexing':
      return 'info';
    default:
      return 'default';
  }
};

export const StatusCard: React.FC<StatusCardProps> = ({ status }) => {
  const progress = status.index 
    ? ((status.index.current - status.index.from) / (status.index.to - status.index.from)) * 100
    : 0;

  return (
    <Card>
      <CardContent>
        <Box display="flex" justifyContent="space-between" alignItems="center" mb={2}>
          <Typography variant="h6" component="h2">
            Indexer Status
          </Typography>
          <Chip 
            label={status.status.toUpperCase()} 
            color={getStatusColor(status.status) as any}
            size="small"
          />
        </Box>
        
        <Box mb={2}>
          <Typography variant="body2" color="text.secondary">
            Current Block: {status.last_block.toLocaleString()}
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Head Block: {status.head.toLocaleString()}
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Behind: {status.behind.toLocaleString()} blocks
          </Typography>
        </Box>

        {status.index && (
          <Box>
            <Typography variant="body2" color="text.secondary" mb={1}>
              {status.index.is_reindex ? 'Reindexing' : 'Indexing'}: {status.index.current.toLocaleString()} / {status.index.to.toLocaleString()}
            </Typography>
            {status.index.strategy && (
              <Typography variant="body2" color="text.secondary" mb={1}>
                Strategy: {status.index.strategy}
              </Typography>
            )}
            <LinearProgress 
              variant="determinate" 
              value={progress} 
              sx={{ mb: 1 }}
            />
            <Typography variant="caption" color="text.secondary">
              {progress.toFixed(1)}% complete
            </Typography>
          </Box>
        )}
      </CardContent>
    </Card>
  );
};