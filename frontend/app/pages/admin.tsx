import bytes from 'bytes';
import {
  ActivityIcon,
  BuildingIcon,
  DatabaseIcon,
  DownloadIcon,
  FileIcon,
  GaugeIcon,
  GlobeIcon,
  HardDriveIcon,
  KeyRoundIcon,
  LoaderCircle,
  ServerIcon,
  TriangleAlertIcon,
  UsersIcon,
  WebhookIcon,
  ZapIcon,
} from 'lucide-react';
import { useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router';
import { Bar, BarChart, CartesianGrid, Cell, Pie, PieChart, XAxis, YAxis } from 'recharts';
import { useQuery } from '@tanstack/react-query';
import apiGetUserAdminBandwidth from '~/api/user/admin/bandwidth.ts';
import apiGetUserAdminFiles from '~/api/user/admin/files.ts';
import apiGetUserAdminNodes from '~/api/user/admin/nodes.ts';
import apiGetUserAdminRatelimits from '~/api/user/admin/ratelimits.ts';
import apiGetUserAdminRequests from '~/api/user/admin/requests.ts';
import apiGetUserAdminStats from '~/api/user/admin/stats.ts';
import { AdminWindow, NodeStatus, SystemSnapshot } from '~/api/user/admin/types.ts';
import { Badge } from '~/components/ui/badge.tsx';
import { Button } from '~/components/ui/button.tsx';
import { Card } from '~/components/ui/card.tsx';
import { ChartContainer, ChartTooltip, ChartTooltipContent } from '~/components/ui/chart.tsx';
import { Input } from '~/components/ui/input.tsx';
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationNext,
  PaginationPrevious,
} from '~/components/ui/pagination.tsx';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '~/components/ui/select.tsx';
import { Skeleton } from '~/components/ui/skeleton.tsx';
import { useAuth } from '~/hooks/use-auth.tsx';

const LIVE_REFETCH_MS = 10_000;
const PER_PAGE = 25;

const SECTIONS = [
  { id: 'overview', label: 'Overview', icon: GaugeIcon },
  { id: 'nodes', label: 'Nodes', icon: ServerIcon },
  { id: 'ratelimits', label: 'Ratelimits', icon: ZapIcon },
  { id: 'bandwidth', label: 'Bandwidth', icon: DownloadIcon },
  { id: 'requests', label: 'Requests', icon: ActivityIcon },
  { id: 'files', label: 'Files', icon: FileIcon },
] as const;

const WINDOWS: { value: AdminWindow; label: string }[] = [
  { value: 'hour', label: 'Last Hour' },
  { value: 'day', label: 'Last Day' },
  { value: 'week', label: 'Last Week' },
  { value: 'month', label: 'Last Month' },
];

const BUCKET_LABELS: Record<string, string> = {
  regular: 'API',
  files: 'File Browsing',
  files_download: 'File Downloads',
};

const formatBytes = (value: number) => bytes(value) ?? '0B';

const formatDuration = (seconds: number) => {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;

  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
};

const formatTime = (unix: number, window: AdminWindow) =>
  new Date(unix * 1000).toLocaleString(undefined, {
    hour: window === 'hour' || window === 'day' ? 'numeric' : undefined,
    minute: window === 'hour' || window === 'day' ? '2-digit' : undefined,
    day: window === 'hour' ? undefined : 'numeric',
    month: window === 'hour' ? undefined : 'short',
  });

function StatCard({
  icon: Icon,
  label,
  value,
  hint,
}: {
  icon: typeof GaugeIcon;
  label: string;
  value: string | number | undefined;
  hint?: string;
}) {
  return (
    <Card className={'p-4 flex flex-row items-center justify-between'}>
      <Icon className={'w-8 h-8 text-muted-foreground shrink-0'} />

      <div className={'flex flex-col text-right items-end min-w-0'}>
        <h1 className={'text-xl font-semibold truncate'}>
          {value === undefined ? (
            <Skeleton className={'w-20 h-7'} />
          ) : typeof value === 'number' ? (
            value.toLocaleString()
          ) : (
            value
          )}
        </h1>
        <p className={'text-sm text-muted-foreground truncate'}>{label}</p>
        {hint && <p className={'text-xs text-muted-foreground/70 truncate'}>{hint}</p>}
      </div>
    </Card>
  );
}

function NodeStrip({ nodes }: { nodes: NodeStatus[] | undefined }) {
  if (!nodes?.length) return null;

  const failed = nodes.filter((node) => node.error);

  return (
    <Card className={'p-3 mb-2'}>
      <div className={'flex flex-row flex-wrap items-center gap-2'}>
        <span className={'text-sm text-muted-foreground mr-1'}>Aggregated from</span>

        {nodes.map((node) => (
          <Badge
            key={node.node}
            className={node.error ? 'bg-red-500 hover:bg-red-400' : 'bg-green-500 hover:bg-green-400'}
            title={node.error ?? undefined}
          >
            {node.node}
            {node.local && ' (this node)'}
            {node.latency !== null && !node.local && ` · ${node.latency}ms`}
          </Badge>
        ))}

        {failed.length > 0 && (
          <span className={'text-xs text-red-400 flex flex-row items-center'}>
            <TriangleAlertIcon size={14} className={'mr-1'} />
            {failed.length} node{failed.length === 1 ? '' : 's'} did not answer — totals are incomplete
          </span>
        )}
      </div>
    </Card>
  );
}

function SystemCard({ system, title }: { system: SystemSnapshot; title: string }) {
  const cacheTotal = system.cacheHits + system.cacheMisses;
  const hitRate = cacheTotal === 0 ? 0 : (system.cacheHits / cacheTotal) * 100;
  const fileCacheUsage =
    system.fileCacheMaxSize === 0 ? 0 : (system.fileCacheSize / system.fileCacheMaxSize) * 100;

  return (
    <Card className={'p-4'}>
      <div className={'flex flex-row items-center justify-between mb-3'}>
        <h2 className={'text-lg font-semibold'}>{title}</h2>
        <Badge variant={'secondary'}>{system.version}</Badge>
      </div>

      <div className={'grid grid-cols-2 md:grid-cols-4 gap-2'}>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>{formatDuration(system.uptime)}</p>
          <p className={'text-xs text-muted-foreground'}>Uptime</p>
        </div>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>
            {system.idleReadConnections} / {system.idleWriteConnections}
          </p>
          <p className={'text-xs text-muted-foreground'}>Idle Pool (read/write)</p>
        </div>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>{hitRate.toFixed(1)}%</p>
          <p className={'text-xs text-muted-foreground'}>
            Cache Hit Rate ({system.cacheHits.toLocaleString()} hits)
          </p>
        </div>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>
            {system.cacheMemory === null ? 'unknown' : formatBytes(system.cacheMemory)}
          </p>
          <p className={'text-xs text-muted-foreground'}>Valkey Memory</p>
        </div>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>{system.fileCacheFiles.toLocaleString()}</p>
          <p className={'text-xs text-muted-foreground'}>Cached Files</p>
        </div>
        <div className={'flex flex-col col-span-2'}>
          <p className={'text-sm font-semibold'}>
            {formatBytes(system.fileCacheSize)} / {formatBytes(system.fileCacheMaxSize)}
          </p>
          <div className={'h-1.5 w-full rounded-full bg-muted mt-1'}>
            <div
              className={'h-1.5 rounded-full bg-primary'}
              style={{ width: `${Math.min(fileCacheUsage, 100)}%` }}
            />
          </div>
          <p className={'text-xs text-muted-foreground mt-1'}>File Cache</p>
        </div>
        <div className={'flex flex-col'}>
          <p className={'text-sm font-semibold'}>
            {system.pendingRequests.toLocaleString()} / {system.pendingFileRequests.toLocaleString()}
          </p>
          <p className={'text-xs text-muted-foreground'}>Queued (api/files)</p>
        </div>
      </div>
    </Card>
  );
}

function TopList({ title, entries }: { title: string; entries: { label: string; value: string; sub?: string }[] }) {
  return (
    <Card className={'p-4'}>
      <h2 className={'text-lg font-semibold mb-2'}>{title}</h2>

      {!entries.length ? (
        <p className={'text-sm text-muted-foreground'}>No data in this window.</p>
      ) : (
        <div className={'flex flex-col'}>
          {entries.map((entry) => (
            <div
              key={entry.label}
              className={'flex flex-row items-center justify-between py-1.5 border-b border-border last:border-0 gap-2'}
            >
              <span className={'text-sm truncate font-mono'} title={entry.label}>
                {entry.label || '(none)'}
              </span>
              <span className={'text-sm shrink-0 text-right'}>
                {entry.value}
                {entry.sub && <span className={'text-xs text-muted-foreground ml-2'}>{entry.sub}</span>}
              </span>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

export default function PageAdmin() {
  const [user, , isUserLoading] = useAuth();

  const [searchParams, setSearchParams] = useSearchParams();

  const setParams = useCallback(
    (updates: Record<string, string | number | null | undefined>) => {
      setSearchParams(
        (prev) => {
          const params = new URLSearchParams(prev);

          for (const [name, value] of Object.entries(updates)) {
            if (value === null || value === undefined || value === '') params.delete(name);
            else params.set(name, String(value));
          }

          return params;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );
  const setParam = useCallback(
    (name: string, value: string | number | null | undefined) => setParams({ [name]: value }),
    [setParams],
  );

  const section = searchParams.get('section') ?? 'overview';
  const search = searchParams.get('search') ?? '';
  const page = Number(searchParams.get('page') ?? '1') || 1;
  const window = (searchParams.get('window') ?? 'day') as AdminWindow;

  const isAdmin = Boolean(user?.admin);

  const { data: overview } = useQuery({
    queryKey: ['admin', 'stats'],
    queryFn: () => apiGetUserAdminStats(),
    enabled: isAdmin,
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const { data: nodes } = useQuery({
    queryKey: ['admin', 'nodes'],
    queryFn: () => apiGetUserAdminNodes(),
    enabled: isAdmin && (section === 'nodes' || section === 'overview'),
    staleTime: 0,
    refetchInterval: LIVE_REFETCH_MS,
  });

  const { data: ratelimits, isFetching: ratelimitsFetching } = useQuery({
    queryKey: ['admin', 'ratelimits', page, search],
    queryFn: () => apiGetUserAdminRatelimits(page, PER_PAGE, search),
    enabled: isAdmin && section === 'ratelimits',
    staleTime: 0,
    refetchInterval: LIVE_REFETCH_MS,
  });

  const { data: bandwidth, isFetching: bandwidthFetching } = useQuery({
    queryKey: ['admin', 'bandwidth', page, search],
    queryFn: () => apiGetUserAdminBandwidth(page, PER_PAGE, search),
    enabled: isAdmin && section === 'bandwidth',
    staleTime: 0,
    refetchInterval: LIVE_REFETCH_MS,
  });

  const { data: requests } = useQuery({
    queryKey: ['admin', 'requests', window],
    queryFn: () => apiGetUserAdminRequests(window),
    enabled: isAdmin && section === 'requests',
    staleTime: 30_000,
  });

  const { data: files } = useQuery({
    queryKey: ['admin', 'files', window],
    queryFn: () => apiGetUserAdminFiles(window),
    enabled: isAdmin && section === 'files',
    staleTime: 30_000,
  });

  const statusChart = useMemo(
    () =>
      (requests?.statuses ?? []).map((entry) => ({
        label: String(entry.status),
        total: entry.total,
      })),
    [requests],
  );

  const requestTimeline = useMemo(
    () =>
      (requests?.timeline ?? []).map((point) => ({
        ...point,
        label: formatTime(point.time, window),
      })),
    [requests, window],
  );

  const filesTimeline = useMemo(
    () =>
      (files?.timeline ?? []).map((point) => ({
        ...point,
        label: formatTime(point.time, window),
      })),
    [files, window],
  );

  if (isUserLoading) {
    return (
      <div className={'w-full flex flex-col gap-2'}>
        <Skeleton className={'w-full h-12 rounded-xl'} />
        <Skeleton className={'w-full h-32 rounded-xl'} />
        <Skeleton className={'w-full h-32 rounded-xl'} />
      </div>
    );
  }

  if (!isAdmin) {
    return (
      <Card className={'p-6 flex flex-col items-center justify-center'}>
        <KeyRoundIcon className={'w-10 h-10 text-muted-foreground mb-2'} />
        <h1 className={'text-xl font-semibold'}>Administrators only</h1>
        <p className={'text-sm text-muted-foreground'}>
          This page is restricted to MCJars administrators.
        </p>
      </Card>
    );
  }

  const windowSelect = (
    <Select value={window} onValueChange={(value) => setParam('window', value)}>
      <SelectTrigger className={'w-[10em]'}>
        <SelectValue placeholder={'Window'} />
      </SelectTrigger>
      <SelectContent>
        {WINDOWS.map((entry) => (
          <SelectItem key={entry.value} value={entry.value}>
            {entry.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );

  const renderPagination = (total: number) => {
    const pages = Math.max(1, Math.ceil(total / PER_PAGE));

    return (
      <div className={'flex flex-row flex-wrap items-center justify-end gap-2 mt-2'}>
        <span className={'mr-auto text-sm text-muted-foreground'}>
          {total.toLocaleString()} entr{total === 1 ? 'y' : 'ies'}
        </span>

        {pages > 1 && (
          <Pagination className={'mx-0 w-fit justify-end'}>
            <PaginationContent>
              <PaginationItem>
                <PaginationPrevious
                  href={'#'}
                  onClick={(e) => {
                    e.preventDefault();
                    if (page > 1) setParam('page', page - 1);
                  }}
                  className={page <= 1 ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                />
              </PaginationItem>
              <PaginationItem>
                <span className={'px-3 text-sm'}>
                  {page} / {pages}
                </span>
              </PaginationItem>
              <PaginationItem>
                <PaginationNext
                  href={'#'}
                  onClick={(e) => {
                    e.preventDefault();
                    if (page < pages) setParam('page', page + 1);
                  }}
                  className={page >= pages ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                />
              </PaginationItem>
            </PaginationContent>
          </Pagination>
        )}
      </div>
    );
  };

  const searchInput = (placeholder: string) => (
    <Input
      value={search}
      placeholder={placeholder}
      className={'md:w-80'}
      onChange={(e) => setParams({ search: e.target.value, page: null })}
    />
  );

  return (
    <div className={'w-full flex flex-col pb-4'}>
      <div className={'flex flex-row flex-wrap gap-1 mb-2'}>
        {SECTIONS.map((entry) => (
          <Button
            key={entry.id}
            variant={section === entry.id ? 'default' : 'secondary'}
            size={'sm'}
            onClick={() =>
              setParams({
                section: entry.id === 'overview' ? null : entry.id,
                page: null,
                search: null,
              })
            }
          >
            <entry.icon size={16} className={'mr-1'} />
            {entry.label}
          </Button>
        ))}
      </div>

      {section === 'overview' && (
        <div className={'flex flex-col gap-2'}>
          <div className={'grid gap-2 grid-cols-1 md:grid-cols-2 xl:grid-cols-4'}>
            <StatCard icon={UsersIcon} label={'Users'} value={overview?.stats.users} />
            <StatCard icon={BuildingIcon} label={'Organizations'} value={overview?.stats.organizations} />
            <StatCard icon={KeyRoundIcon} label={'Sessions'} value={overview?.stats.sessions} />
            <StatCard icon={WebhookIcon} label={'Webhooks'} value={overview?.stats.webhooks} />
            <StatCard
              icon={ServerIcon}
              label={'Nodes Alive'}
              value={
                overview ? `${overview.stats.nodes.alive} / ${overview.stats.nodes.total}` : undefined
              }
            />
            <StatCard icon={DatabaseIcon} label={'Requests (all time)'} value={overview?.stats.requests.total} />
            <StatCard icon={ActivityIcon} label={'Requests (24h)'} value={overview?.stats.requests.day} />
            <StatCard
              icon={ZapIcon}
              label={'Requests / min'}
              value={overview?.stats.requests.minute}
              hint={overview ? `${overview.stats.requests.hour.toLocaleString()} in the last hour` : undefined}
            />
          </div>

          {overview && <SystemCard system={overview.system} title={'This Node'} />}

          {nodes && nodes.length > 1 && (
            <Card className={'p-4'}>
              <h2 className={'text-lg font-semibold mb-2'}>Cluster</h2>
              <div className={'flex flex-row flex-wrap gap-2'}>
                {nodes.map((node) => (
                  <Badge
                    key={node.name}
                    className={node.alive ? 'bg-green-500 hover:bg-green-400' : 'bg-red-500 hover:bg-red-400'}
                  >
                    {node.name}
                    {node.local && ' (this node)'}
                  </Badge>
                ))}
              </div>
            </Card>
          )}
        </div>
      )}

      {section === 'nodes' && (
        <div className={'flex flex-col gap-2'}>
          {!nodes ? (
            <Skeleton className={'w-full h-32 rounded-xl'} />
          ) : !nodes.length ? (
            <Card className={'p-4'}>
              <p className={'text-sm text-muted-foreground'}>
                No nodes have registered. A node only joins the registry once it has both{' '}
                <code>SERVER_NAME</code> and <code>NODE_URL</code> configured.
              </p>
            </Card>
          ) : (
            nodes.map((node) => (
              <Card key={node.name} className={'p-4'}>
                <div className={'flex flex-row flex-wrap items-center justify-between gap-2 mb-2'}>
                  <div className={'flex flex-row items-center gap-2'}>
                    <ServerIcon className={'w-5 h-5 text-muted-foreground'} />
                    <h2 className={'text-lg font-semibold'}>{node.name}</h2>
                    {node.local && <Badge variant={'secondary'}>this node</Badge>}
                    <Badge className={node.alive ? 'bg-green-500 hover:bg-green-400' : 'bg-red-500 hover:bg-red-400'}>
                      {node.alive ? 'alive' : 'down'}
                    </Badge>
                  </div>

                  <div className={'flex flex-row items-center gap-3 text-xs text-muted-foreground'}>
                    {node.latency !== null && <span>{node.latency}ms</span>}
                    <span>last seen {new Date(`${node.lastSeen}Z`).toLocaleString()}</span>
                    {node.url && <span className={'font-mono truncate max-w-60'}>{node.url}</span>}
                  </div>
                </div>

                {node.error ? (
                  <p className={'text-sm text-red-400 flex flex-row items-center'}>
                    <TriangleAlertIcon size={16} className={'mr-1'} />
                    {node.error}
                  </p>
                ) : node.system ? (
                  <SystemCard system={node.system} title={'Runtime'} />
                ) : (
                  <p className={'text-sm text-muted-foreground'}>
                    No runtime data. Peer queries need <code>NODE_SECRET</code> set to the same value on
                    every node.
                  </p>
                )}
              </Card>
            ))
          )}
        </div>
      )}

      {section === 'ratelimits' && (
        <div className={'flex flex-col'}>
          <NodeStrip nodes={ratelimits?.nodes} />

          <Card className={'p-3 mb-2'}>
            <div className={'flex flex-row flex-wrap items-center justify-between gap-2'}>
              {searchInput('Filter by IP address...')}

              <p className={'text-xs text-muted-foreground'}>
                Limits are enforced per node, per minute. Verified organizations are exempt and never
                appear here.
              </p>
            </div>
          </Card>

          {!ratelimits ? (
            <Skeleton className={'w-full h-32 rounded-xl'} />
          ) : !ratelimits.ratelimits.data.length ? (
            <Card className={'p-4 flex flex-row items-center justify-center'}>
              {ratelimitsFetching ? (
                <LoaderCircle className={'animate-spin'} />
              ) : (
                <p className={'text-sm text-muted-foreground'}>No active ratelimit counters.</p>
              )}
            </Card>
          ) : (
            <>
              {ratelimits.ratelimits.data.map((entry) => {
                const usage = (entry.hits / entry.limit) * 100;

                return (
                  <Card key={`${entry.ip}-${entry.bucket}`} className={'p-3 mb-2'}>
                    <div className={'flex flex-row flex-wrap items-center justify-between gap-2'}>
                      <div className={'flex flex-row items-center gap-2 min-w-0'}>
                        <GlobeIcon className={'w-4 h-4 text-muted-foreground shrink-0'} />
                        <span className={'font-mono text-sm truncate'}>{entry.ip}</span>
                        <Badge variant={'secondary'}>{BUCKET_LABELS[entry.bucket] ?? entry.bucket}</Badge>
                      </div>

                      <div className={'flex flex-row items-center gap-4'}>
                        <div className={'flex flex-col items-end'}>
                          <span className={'text-sm font-semibold'}>
                            {entry.hits.toLocaleString()} / {entry.limit.toLocaleString()}
                          </span>
                          <span className={'text-xs text-muted-foreground'}>resets in {entry.reset}s</span>
                        </div>

                        <div className={'w-24 h-1.5 rounded-full bg-muted'}>
                          <div
                            className={usage >= 100 ? 'h-1.5 rounded-full bg-red-500' : 'h-1.5 rounded-full bg-primary'}
                            style={{ width: `${Math.min(usage, 100)}%` }}
                          />
                        </div>
                      </div>
                    </div>

                    {Object.keys(entry.nodes).length > 1 && (
                      <div className={'flex flex-row flex-wrap gap-1 mt-2'}>
                        {Object.entries(entry.nodes).map(([node, hits]) => (
                          <Badge key={node} variant={'secondary'} className={'text-xs'}>
                            {node}: {hits.toLocaleString()}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </Card>
                );
              })}

              {renderPagination(ratelimits.ratelimits.total)}
            </>
          )}
        </div>
      )}

      {section === 'bandwidth' && (
        <div className={'flex flex-col'}>
          <NodeStrip nodes={bandwidth?.nodes} />

          <Card className={'p-3 mb-2'}>
            <div className={'flex flex-row flex-wrap items-center justify-between gap-2'}>
              {searchInput('Filter by IP or organization...')}

              <p className={'text-xs text-muted-foreground'}>
                Daily download allowance, counted per node. Only <code>.jar</code> and <code>.zip</code>{' '}
                downloads count toward it.
              </p>
            </div>
          </Card>

          {!bandwidth ? (
            <Skeleton className={'w-full h-32 rounded-xl'} />
          ) : !bandwidth.bandwidth.data.length ? (
            <Card className={'p-4 flex flex-row items-center justify-center'}>
              {bandwidthFetching ? (
                <LoaderCircle className={'animate-spin'} />
              ) : (
                <p className={'text-sm text-muted-foreground'}>No bandwidth counters active today.</p>
              )}
            </Card>
          ) : (
            <>
              {bandwidth.bandwidth.data.map((entry) => {
                const usage = (entry.used / entry.limit) * 100;
                const key = entry.organizationId !== null ? `org-${entry.organizationId}` : `ip-${entry.ip}`;

                return (
                  <Card key={key} className={'p-3 mb-2'}>
                    <div className={'flex flex-row flex-wrap items-center justify-between gap-2'}>
                      <div className={'flex flex-row items-center gap-2 min-w-0'}>
                        {entry.organizationId !== null ? (
                          <BuildingIcon className={'w-4 h-4 text-muted-foreground shrink-0'} />
                        ) : (
                          <GlobeIcon className={'w-4 h-4 text-muted-foreground shrink-0'} />
                        )}
                        <span className={'font-mono text-sm truncate'}>
                          {entry.organizationId !== null
                            ? (entry.organizationName ?? `organization #${entry.organizationId}`)
                            : entry.ip}
                        </span>
                        {entry.verified && <Badge className={'bg-blue-500 hover:bg-blue-400'}>verified</Badge>}
                      </div>

                      <div className={'flex flex-row items-center gap-4'}>
                        <div className={'flex flex-col items-end'}>
                          <span className={'text-sm font-semibold'}>
                            {formatBytes(entry.used)} / {formatBytes(entry.limit)}
                          </span>
                          <span className={'text-xs text-muted-foreground'}>
                            resets in {formatDuration(entry.reset)}
                          </span>
                        </div>

                        <div className={'w-24 h-1.5 rounded-full bg-muted'}>
                          <div
                            className={usage >= 100 ? 'h-1.5 rounded-full bg-red-500' : 'h-1.5 rounded-full bg-primary'}
                            style={{ width: `${Math.min(usage, 100)}%` }}
                          />
                        </div>
                      </div>
                    </div>

                    {Object.keys(entry.nodes).length > 1 && (
                      <div className={'flex flex-row flex-wrap gap-1 mt-2'}>
                        {Object.entries(entry.nodes).map(([node, used]) => (
                          <Badge key={node} variant={'secondary'} className={'text-xs'}>
                            {node}: {formatBytes(used)}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </Card>
                );
              })}

              {renderPagination(bandwidth.bandwidth.total)}
            </>
          )}
        </div>
      )}

      {section === 'requests' && (
        <div className={'flex flex-col gap-2'}>
          <div className={'flex flex-row items-center justify-end'}>{windowSelect}</div>

          <div className={'grid gap-2 grid-cols-1 md:grid-cols-2 xl:grid-cols-5'}>
            <StatCard icon={ActivityIcon} label={'Requests'} value={requests?.total} />
            <StatCard icon={GlobeIcon} label={'Unique IPs'} value={requests?.uniqueIps} />
            <StatCard
              icon={TriangleAlertIcon}
              label={'Errors'}
              value={requests?.errors}
              hint={
                requests && requests.total > 0
                  ? `${((requests.errors / requests.total) * 100).toFixed(2)}% of requests`
                  : undefined
              }
            />
            <StatCard
              icon={GaugeIcon}
              label={'Average Time'}
              value={requests ? `${requests.avgTime.toFixed(1)}ms` : undefined}
            />
            <StatCard
              icon={GaugeIcon}
              label={'p95 Time'}
              value={requests ? `${requests.p95Time.toFixed(1)}ms` : undefined}
            />
          </div>

          <Card className={'p-4 h-[400px]'}>
            <h2 className={'text-lg font-semibold mb-2'}>Requests Over Time</h2>

            {!requestTimeline.length ? (
              <div className={'w-full h-full flex flex-row items-center justify-center'}>
                <LoaderCircle className={'animate-spin'} />
              </div>
            ) : (
              <ChartContainer config={{}} className={'w-full h-[320px]'}>
                <BarChart data={requestTimeline} layout={'horizontal'}>
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <CartesianGrid vertical={false} />
                  <YAxis dataKey={'total'} type={'number'} />
                  <XAxis dataKey={'label'} type={'category'} />
                  <Bar fill={'hsl(var(--chart-1))'} dataKey={'total'} radius={2} />
                </BarChart>
              </ChartContainer>
            )}
          </Card>

          <div className={'grid gap-2 grid-cols-1 xl:grid-cols-2'}>
            <Card className={'p-4 h-[400px]'}>
              <h2 className={'text-lg font-semibold mb-2'}>Status Codes</h2>

              {!statusChart.length ? (
                <div className={'w-full h-full flex flex-row items-center justify-center'}>
                  <LoaderCircle className={'animate-spin'} />
                </div>
              ) : (
                <ChartContainer config={{}} className={'w-full h-[320px]'}>
                  <PieChart accessibilityLayer>
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Pie
                      data={statusChart}
                      dataKey={'total'}
                      nameKey={'label'}
                      fillRule={'evenodd'}
                      label={({ name }) => name}
                    >
                      {statusChart.map(({ label }, i) => (
                        <Cell key={label} fill={`hsl(var(--chart-${(i % 5) + 1}))`} stroke={'hsl(var(--border))'} />
                      ))}
                    </Pie>
                  </PieChart>
                </ChartContainer>
              )}
            </Card>

            <TopList
              title={'Slowest Paths (p95)'}
              entries={(requests?.slowest ?? []).map((entry) => ({
                label: entry.path,
                value: `${entry.p95Time.toFixed(0)}ms`,
                sub: `${entry.total.toLocaleString()} reqs`,
              }))}
            />

            <TopList
              title={'Top IPs'}
              entries={(requests?.ips ?? []).map((entry) => ({
                label: entry.ip,
                value: entry.total.toLocaleString(),
                sub: `${entry.country ?? '??'} · ${entry.userAgents} UA`,
              }))}
            />

            <TopList
              title={'Top User Agents'}
              entries={(requests?.userAgents ?? []).map((entry) => ({
                label: entry.label,
                value: entry.total.toLocaleString(),
                sub: `${entry.uniqueIps.toLocaleString()} IPs`,
              }))}
            />

            <TopList
              title={'Top Origins'}
              entries={(requests?.origins ?? []).map((entry) => ({
                label: entry.label,
                value: entry.total.toLocaleString(),
                sub: `${entry.uniqueIps.toLocaleString()} IPs`,
              }))}
            />

            <TopList
              title={'Top Countries'}
              entries={(requests?.countries ?? []).map((entry) => ({
                label: entry.label,
                value: entry.total.toLocaleString(),
                sub: `${entry.uniqueIps.toLocaleString()} IPs`,
              }))}
            />
          </div>
        </div>
      )}

      {section === 'files' && (
        <div className={'flex flex-col gap-2'}>
          <div className={'flex flex-row items-center justify-end'}>{windowSelect}</div>

          <div className={'grid gap-2 grid-cols-1 md:grid-cols-2 xl:grid-cols-4'}>
            <StatCard
              icon={DownloadIcon}
              label={'Bytes Served'}
              value={files ? formatBytes(files.bytesSent) : undefined}
            />
            <StatCard icon={FileIcon} label={'File Requests'} value={files?.requests} />
            <StatCard
              icon={HardDriveIcon}
              label={'Cache Hit Rate'}
              value={files ? `${(files.cacheHitRate * 100).toFixed(1)}%` : undefined}
              hint={files ? `${files.cacheHits.toLocaleString()} hits` : undefined}
            />
            <StatCard
              icon={GaugeIcon}
              label={'Average Time'}
              value={files ? `${files.avgTime.toFixed(1)}ms` : undefined}
            />
          </div>

          <Card className={'p-4 h-[400px]'}>
            <h2 className={'text-lg font-semibold mb-2'}>Bandwidth Over Time</h2>

            {!filesTimeline.length ? (
              <div className={'w-full h-full flex flex-row items-center justify-center'}>
                <LoaderCircle className={'animate-spin'} />
              </div>
            ) : (
              <ChartContainer config={{}} className={'w-full h-[320px]'}>
                <BarChart data={filesTimeline} layout={'horizontal'}>
                  <ChartTooltip
                    content={<ChartTooltipContent formatter={(value) => formatBytes(Number(value))} />}
                  />
                  <CartesianGrid vertical={false} />
                  <YAxis dataKey={'bytes'} type={'number'} tickFormatter={(value) => formatBytes(Number(value))} />
                  <XAxis dataKey={'label'} type={'category'} />
                  <Bar fill={'hsl(var(--chart-2))'} dataKey={'bytes'} radius={2} />
                </BarChart>
              </ChartContainer>
            )}
          </Card>

          <Card className={'p-3'}>
            <p className={'text-xs text-muted-foreground'}>
              The breakdowns below are all-time totals from the analytics rollup, refreshed every 30
              minutes. The figures above cover the selected window and are live.
            </p>
          </Card>

          <div className={'grid gap-2 grid-cols-1 xl:grid-cols-2'}>
            <TopList
              title={'By Root'}
              entries={(files?.roots ?? []).map((entry) => ({
                label: entry.label,
                value: formatBytes(entry.bytes),
                sub: `${entry.requests.toLocaleString()} reqs`,
              }))}
            />

            <TopList
              title={'By Kind'}
              entries={(files?.kinds ?? []).map((entry) => ({
                label: entry.label,
                value: formatBytes(entry.bytes),
                sub: `${entry.requests.toLocaleString()} reqs`,
              }))}
            />

            <TopList
              title={'By Extension'}
              entries={(files?.extensions ?? []).map((entry) => ({
                label: entry.label,
                value: formatBytes(entry.bytes),
                sub: `${entry.requests.toLocaleString()} reqs`,
              }))}
            />

            <TopList
              title={'Most Requested Files'}
              entries={(files?.top ?? []).map((entry) => ({
                label: entry.label,
                value: entry.requests.toLocaleString(),
                sub: formatBytes(entry.bytes),
              }))}
            />
          </div>
        </div>
      )}
    </div>
  );
}
