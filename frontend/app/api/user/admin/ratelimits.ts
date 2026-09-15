import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { NodeStatus, Paginated } from '~/api/user/admin/types.ts';

export type RateLimitBucket = 'regular' | 'files' | 'files_download';

export type AdminRateLimit = {
  ip: string;
  bucket: RateLimitBucket;

  hits: number;
  limit: number;
  reset: number;

  nodes: Record<string, number>;
};

export default async function apiGetUserAdminRatelimits(
  page: number,
  perPage: number,
  search?: string,
): Promise<{ nodes: NodeStatus[]; ratelimits: Paginated<AdminRateLimit> }> {
  const { data } = await axios.get<{
    nodes: NodeStatus[];
    ratelimits: Paginated<AdminRateLimit>;
  }>(`${BASE_URL}/api/user/admin/ratelimits`, {
    params: { page, per_page: perPage, search: search || undefined },
    withCredentials: true,
  });

  return { nodes: data.nodes, ratelimits: data.ratelimits };
}
