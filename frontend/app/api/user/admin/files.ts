import axios from 'axios';
import { BASE_URL } from '~/api/index.ts';
import { AdminWindow } from '~/api/user/admin/types.ts';

export type BandwidthPoint = {
  time: number;
  bytes: number;
  requests: number;
  cacheHits: number;
};

export type Breakdown = {
  label: string;
  requests: number;
  uniqueIps: number;
  bytes: number;
};

export type AdminFiles = {
  window: AdminWindow;

  bytesSent: number;
  requests: number;
  cacheHits: number;
  cacheHitRate: number;
  avgTime: number;

  timeline: BandwidthPoint[];
  roots: Breakdown[];
  kinds: Breakdown[];
  extensions: Breakdown[];
  top: Breakdown[];
};

export default async function apiGetUserAdminFiles(window: AdminWindow): Promise<AdminFiles> {
  const { data } = await axios.get<{
    files: AdminFiles;
  }>(`${BASE_URL}/api/user/admin/files`, {
    params: { window },
    withCredentials: true,
  });

  return data.files;
}
