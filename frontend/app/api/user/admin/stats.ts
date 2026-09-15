import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { SystemSnapshot } from '~/api/user/admin/types.ts';

export type AdminStats = {
  organizations: number;
  users: number;
  sessions: number;
  webhooks: number;

  nodes: {
    total: number;
    alive: number;
  };

  requests: {
    total: number;
    minute: number;
    hour: number;
    day: number;
    week: number;
    month: number;
    year: number;
  };
};

export default async function apiGetUserAdminStats(): Promise<{ stats: AdminStats; system: SystemSnapshot }> {
  const { data } = await axios.get<{
    stats: AdminStats;
    system: SystemSnapshot;
  }>(`${BASE_URL}/api/user/admin/stats`, {
    withCredentials: true,
  });

  return { stats: data.stats, system: data.system };
}
