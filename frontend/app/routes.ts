import { type RouteConfig, index, route } from '@react-router/dev/routes';

export default [
  index('pages/index.tsx'),
  route('configs', 'pages/config.tsx'),
  route('lookup', 'pages/lookup.tsx'),
  route('job-status', 'pages/job-status.tsx'),
  route('organizations', 'pages/organizations.tsx'),
  route('admin', 'pages/admin.tsx'),
  route(':type/config', 'pages/{type}/config.tsx'),
  route(':type/versions', 'pages/{type}/versions.tsx'),
  route(':type/statistics', 'pages/{type}/statistics.tsx'),
  route('*', 'pages/404.tsx'),
] satisfies RouteConfig;
