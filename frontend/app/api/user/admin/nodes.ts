import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { SystemSnapshot } from '~/api/user/admin/types.ts';

export type AdminNode = {
  name: string;
  url: string;
  version: string;

  local: boolean;
  alive: boolean;
  latency: number | null;

  lastSeen: string;
  created: string;

  system: SystemSnapshot | null;
  error: string | null;
};

export default async function apiGetUserAdminNodes(): Promise<AdminNode[]> {
  const { data } = await axios.get<{
    nodes: AdminNode[];
  }>(`${BASE_URL}/api/user/admin/nodes`, {
    withCredentials: true,
  });

  return data.nodes;
}
