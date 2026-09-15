import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { NodeStatus, Paginated } from '~/api/user/admin/types.ts';

export type AdminBandwidth = {
  ip: string | null;
  organizationId: number | null;
  organizationName: string | null;
  verified: boolean;

  used: number;
  limit: number;
  reset: number;

  nodes: Record<string, number>;
};

export default async function apiGetUserAdminBandwidth(
  page: number,
  perPage: number,
  search?: string,
): Promise<{ nodes: NodeStatus[]; bandwidth: Paginated<AdminBandwidth> }> {
  const { data } = await axios.get<{
    nodes: NodeStatus[];
    bandwidth: Paginated<AdminBandwidth>;
  }>(`${BASE_URL}/api/user/admin/bandwidth`, {
    params: { page, per_page: perPage, search: search || undefined },
    withCredentials: true,
  });

  return { nodes: data.nodes, bandwidth: data.bandwidth };
}
