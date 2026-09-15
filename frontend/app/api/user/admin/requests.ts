import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { AdminWindow } from '~/api/user/admin/types.ts';

export type TimelinePoint = {
  time: number;
  total: number;
  uniqueIps: number;
  errors: number;
  avgTime: number;
};

export type TopEntry = {
  label: string;
  total: number;
  uniqueIps: number;
};

export type TopIp = {
  ip: string;
  total: number;
  errors: number;
  userAgents: number;
  country: string | null;
};

export type AdminRequests = {
  window: AdminWindow;

  total: number;
  uniqueIps: number;
  errors: number;
  avgTime: number;
  p95Time: number;

  timeline: TimelinePoint[];
  statuses: { status: number; total: number }[];
  slowest: { path: string; total: number; avgTime: number; p95Time: number }[];
  ips: TopIp[];
  userAgents: TopEntry[];
  origins: TopEntry[];
  countries: TopEntry[];
};

export default async function apiGetUserAdminRequests(window: AdminWindow): Promise<AdminRequests> {
  const { data } = await axios.get<{
    requests: AdminRequests;
  }>(`${BASE_URL}/api/user/admin/requests`, {
    params: { window },
    withCredentials: true,
  });

  return data.requests;
}
