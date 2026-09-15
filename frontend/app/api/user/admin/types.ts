export type NodeStatus = {
  node: string;
  local: boolean;
  latency: number | null;
  error: string | null;
};

export type SystemSnapshot = {
  version: string;
  uptime: number;
  idleReadConnections: number;
  idleWriteConnections: number;
  cacheHits: number;
  cacheMisses: number;
  cacheMemory: number | null;
  fileCacheFiles: number;
  fileCacheSize: number;
  fileCacheMaxSize: number;
  pendingRequests: number;
  pendingFileRequests: number;
};

export type Paginated<T> = {
  total: number;
  perPage: number;
  page: number;
  data: T[];
};

export type AdminWindow = 'hour' | 'day' | 'week' | 'month';
