import { isNotNull, relations, sql } from "drizzle-orm"
import { foreignKey, index, integer, uuid, primaryKey, pgTable, varchar, uniqueIndex, pgEnum, serial, jsonb, char, boolean, smallint, timestamp, inet, text, customType, bigint, date } from "drizzle-orm/pg-core"

export const bytea = customType<{ data: string; notNull: false; default: false }>({
  dataType() {
    return 'bytea'
  },

  toDriver(val: string) {
    let newVal = val
    if (val.startsWith('0x')) {
      newVal = val.slice(2)
    }

    return Buffer.from(newVal, 'hex')
  },

  fromDriver(val: unknown) {
    return (val as Buffer).toString('hex')
  }
})

export const types = [
	'VANILLA',
	'PAPER',
	'PUFFERFISH',
	'SPIGOT',
	'FOLIA',
	'PURPUR',
	'WATERFALL',
	'VELOCITY',
	'FABRIC',
	'BUNGEECORD',
	'QUILT',
	'FORGE',
	'NEOFORGE',
	'MOHIST',
	'ARCLIGHT',
	'SPONGE',
	'LEAVES',
	'CANVAS',
	'ASPAPER',
	'LEGACY_FABRIC',
	'LOOHP_LIMBO',
	'NANOLIMBO',
	'DIVINEMC',
	'MAGMA',
	'LEAF',
	'VELOCITY_CTD',
	'YOUER',
	'PLUTO'
] as const

export const formats = [
	'YAML',
	'CONF',
	'TOML',
	'PROPERTIES',
	'JSON5'
] as const

export const typesEnum = pgEnum('server_type', types),
	versionTypeEnum = pgEnum('version_type', ['RELEASE', 'SNAPSHOT']),
	formatsEnum = pgEnum('format', formats),
	methodEnum = pgEnum('method', ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'])

export type ServerType = typeof types[number]
export type Format = typeof formats[number]
export type Method = typeof methodEnum['enumValues'][number]

export type InstallStep = {
	type: 'download'

	file: string
	url: string
	size: number
} | {
	type: 'unzip'

	file: string
	location: string
} | {
	type: 'remove'

	location: string
}

export const organizations = pgTable('organizations', {
	id: serial('id').primaryKey().notNull(),
	ownerId: integer('owner_id').notNull().references(() => users.id, { onDelete: 'cascade' }),

	verified: boolean('verified').default(false).notNull(),
	public: boolean('public').default(false).notNull(),

	name: varchar('name', { length: 255 }).notNull(),
	icon: varchar('icon', { length: 255 }).default('https://s3.mcjars.app/organization-icons/default.webp').notNull(),
	types: jsonb('types').default([]).$type<ServerType[]>().notNull(),

	created: timestamp('created').default(sql`now()`).notNull()
}, (organizations) => [
	uniqueIndex('organizations_name_idx').on(organizations.name)
])

export const organizationKeys = pgTable('organization_keys', {
	id: serial('id').primaryKey().notNull(),
	organizationId: integer('organization_id').notNull().references(() => organizations.id, { onDelete: 'cascade' }),

	name: varchar('name', { length: 255 }).notNull().default('Key'),
	keyId: char('key_id', { length: 16 }).notNull(),
	key: text('key').notNull().unique(),

	created: timestamp('created').default(sql`now()`).notNull()
}, (organizationKeys) => [
	uniqueIndex('organizationKeys_organization_name_idx').on(organizationKeys.organizationId, organizationKeys.name),
	index('organizationKeys_organization_idx').on(organizationKeys.organizationId),
	index('organizationKeys_key_id_idx').on(organizationKeys.keyId)
])

export const organizationSubusers = pgTable('organization_subusers', {
	organizationId: integer('organization_id').notNull().references(() => organizations.id, { onDelete: 'cascade' }),
	userId: integer('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
	pending: boolean('pending').default(true).notNull(),
	created: timestamp('created').default(sql`now()`).notNull()
}, (organizationSubusers) => [
	primaryKey({ name: 'organizationSubusers_pk', columns: [organizationSubusers.organizationId, organizationSubusers.userId] }),
	index('organizationSubusers_organization_idx').on(organizationSubusers.organizationId),
	index('organizationSubusers_user_idx').on(organizationSubusers.userId),
	index('organizationSubusers_userId_pending_idx').on(organizationSubusers.userId, organizationSubusers.pending)
])

export const webhooks = pgTable('webhooks', {
	id: serial('id').primaryKey().notNull(),
	organizationId: integer('organization_id').references(() => organizations.id, { onDelete: 'restrict' }),

	label: varchar('label', { length: 255 }),
	url: varchar('url', { length: 255 }).notNull(),
	types: jsonb('types').$type<ServerType[]>().default([...types]).notNull(),
	enabled: boolean('enabled').default(true).notNull(),
	successful: integer('successful').default(0).notNull(),
	failed: integer('failed').default(0).notNull()
}, (webhooks) => [
	index('webhooks_organization_idx').on(webhooks.organizationId).where(isNotNull(webhooks.organizationId)),
	index('webhooks_enabled_idx').on(webhooks.enabled)
])

export const counts = pgTable('counts', {
	key: varchar('key', { length: 255 }).primaryKey().notNull(),
	value: bigint('value', { mode: 'number' }).notNull().default(0)
}, (counts) => [
	uniqueIndex('counts_key_idx').on(counts.key)
])

export const users = pgTable('users', {
	id: serial('id').primaryKey().notNull(),
	githubId: integer('github_id').notNull(),

	admin: boolean('admin').default(false).notNull(),

	name: varchar('name', { length: 255 }),
	email: varchar('email', { length: 255 }).notNull(),
	login: varchar('login', { length: 255 }).notNull(),

	lastLogin: timestamp('last_login').default(sql`now()`).notNull(),
	created: timestamp('created').default(sql`now()`).notNull()
}, (users) => [
	uniqueIndex('users_github_id_idx').on(users.githubId),
	uniqueIndex('users_login_idx').on(users.login),
	index('users_email_idx').on(users.email)
])

export const userSessions = pgTable('user_sessions', {
	id: serial('id').primaryKey().notNull(),
	userId: integer('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),

	keyId: char('key_id', { length: 16 }).notNull(),
	key: text('key').notNull(),

	ip: inet('ip').notNull(),
	userAgent: varchar('user_agent', { length: 255 }).notNull(),

	lastUsed: timestamp('last_used').default(sql`now()`).notNull(),
	created: timestamp('created').default(sql`now()`).notNull()
}, (userSessions) => [
	index('userSessions_user_idx').on(userSessions.userId),
	uniqueIndex('userSessions_key_idx').on(userSessions.key),
	index('userSessions_key_id_idx').on(userSessions.keyId)
])

export const minecraftVersions = pgTable('minecraft_versions', {
	id: varchar('id', { length: 63 }).primaryKey().notNull(),

	type: versionTypeEnum('type').notNull(),
	supported: boolean('supported').notNull(),
	java: smallint('java').default(21).notNull(),

	created: timestamp('created').notNull()
}, (minecraftVersions) => [
	index('minecraftVersions_type_idx').on(minecraftVersions.type),
	index('minecraftVersions_java_idx').on(minecraftVersions.java)
])

export const projectVersions = pgTable('project_versions', {
	id: varchar('id', { length: 63 }).notNull(),

	type: typesEnum('type').notNull()
}, (projectVersions) => [
	primaryKey({ name: 'projectVersions_pk', columns: [projectVersions.type, projectVersions.id] }),

	index('projectVersions_type_idx').on(projectVersions.type)
])

export const minecraftVersionsRelations = relations(minecraftVersions, ({ many }) => ({
	builds: many(builds, { relationName: 'versions' })
}))

export const projectVersionsRelations = relations(projectVersions, ({ many }) => ({
	builds: many(builds, { relationName: 'project_versions' })
}))

export const files = pgTable('files', {
	path: varchar({ length: 255 }).array(),
	size: integer('size').notNull(),

	sha1: bytea('sha1').notNull(),
	sha224: bytea('sha224').notNull(),
	sha256: bytea('sha256').notNull(),
	sha384: bytea('sha384').notNull(),
	sha512: bytea('sha512').notNull(),
	md5: bytea('md5').notNull(),

	lastAccess: timestamp('last_access')
}, (files) => [
	primaryKey({ name: 'files_pk', columns: [files.path] }),

	index('files_sha1_idx').on(files.sha1).with({ fillfactor: 100 }),
	index('files_sha224_idx').on(files.sha224).with({ fillfactor: 100 }),
	index('files_sha256_idx').on(files.sha256).with({ fillfactor: 100 }),
	index('files_sha384_idx').on(files.sha384).with({ fillfactor: 100 }),
	index('files_sha512_idx').on(files.sha512).with({ fillfactor: 100 }),
	index('files_md5_idx').on(files.md5).with({ fillfactor: 100 })
])

export const builds = pgTable('builds', {
	id: serial('id').primaryKey().notNull(),
	uuid: uuid('uuid').notNull().default(sql`gen_random_uuid()`),

	versionId: varchar('version_id', { length: 63 }).references(() => minecraftVersions.id, { onDelete: 'cascade' }),
	projectVersionId: varchar('project_version_id', { length: 63 }),

	type: typesEnum('type').notNull(),
	rehash: boolean('rehash').default(false).notNull(),
	experimental: boolean('experimental').default(false).notNull(),

	name: varchar('name', { length: 255 }).default('').notNull(),
	buildNumber: integer('build_number').notNull(),
	jarUrl: varchar('jar_url', { length: 255 }),
	jarSize: integer('jar_size'),
	jarLocation: varchar('jar_location', { length: 51 }),
	zipUrl: varchar('zip_url', { length: 255 }),
	zipSize: integer('zip_size'),

	metadata: jsonb('metadata').notNull(),
	installation: jsonb('installation').$type<InstallStep[][]>().notNull(),
	changes: jsonb('changes').$type<string[]>().notNull(),
	created: timestamp('created')
}, (builds) => [
	uniqueIndex('builds_uuid_idx').on(builds.uuid),

	index('builds_type_idx').on(builds.type),
	index('builds_experimental_idx').on(builds.experimental),
	index('builds_name_idx').on(builds.name),
	index('builds_build_number_idx').on(builds.buildNumber),
	index('builds_jar_url_idx').on(builds.jarUrl).where(isNotNull(builds.jarUrl)),
	index('builds_jar_size_idx').on(builds.jarSize).where(isNotNull(builds.jarSize)),
	index('builds_zip_url_idx').on(builds.zipUrl).where(isNotNull(builds.zipUrl)),
	index('builds_zip_size_idx').on(builds.zipSize).where(isNotNull(builds.zipSize)),
	index('builds_created_idx').on(builds.created),
	index('builds_version_type_idx').on(builds.versionId, builds.type),
	index('builds_project_version_type_idx').on(builds.projectVersionId, builds.type),
	index('builds_version_idx').on(builds.versionId),
	index('builds_changes_idx').on(builds.changes).where(sql`jsonb_array_length(changes) > 0 AND jsonb_array_length(changes) < 10`),

	foreignKey({
		columns: [builds.type, builds.projectVersionId],
		foreignColumns: [projectVersions.type, projectVersions.id],
		name: 'builds_project_version_fk'
	}).onDelete('cascade')
])

export const buildsRelations = relations(builds, ({ one, many }) => ({
	version: one(minecraftVersions, {
		fields: [builds.versionId],
		references: [minecraftVersions.id],
		relationName: 'versions'
	}),

	projectVersion: one(projectVersions, {
		fields: [builds.type, builds.projectVersionId],
		references: [projectVersions.type, projectVersions.id],
		relationName: 'projectVersions'
	}),

	hashes: many(buildHashes)
}))

export const buildHashes = pgTable('build_hashes', {
	buildId: integer('build_id').notNull().references(() => builds.id, { onDelete: 'cascade' }),

	primary: boolean('primary').notNull(),

	sha1: bytea('sha1').notNull(),
	sha224: bytea('sha224').notNull(),
	sha256: bytea('sha256').notNull(),
	sha384: bytea('sha384').notNull(),
	sha512: bytea('sha512').notNull(),
	md5: bytea('md5').notNull()
}, (hashes) => [
	index('buildHashes_build_idx').on(hashes.buildId),
	index('buildHashes_primary_idx').on(hashes.primary),
	index('buildHashes_sha1_idx').on(hashes.sha1).with({ fillfactor: 90 }),
	index('buildHashes_sha224_idx').on(hashes.sha224).with({ fillfactor: 90 }),
	index('buildHashes_sha256_idx').on(hashes.sha256).with({ fillfactor: 90 }),
	index('buildHashes_sha384_idx').on(hashes.sha384).with({ fillfactor: 90 }),
	index('buildHashes_sha512_idx').on(hashes.sha512).with({ fillfactor: 90 }),
	index('buildHashes_md5_idx').on(hashes.md5).with({ fillfactor: 90 })
])

export const buildHashesRelations = relations(buildHashes, ({ one }) => ({
	build: one(builds, {
		fields: [buildHashes.buildId],
		references: [builds.id]
	})
}))

export const configs = pgTable('configs', {
	id: serial('id').primaryKey().notNull(),
	uuid: uuid('uuid').notNull().default(sql`gen_random_uuid()`),

	location: varchar('location', { length: 51 }).notNull().unique(),
	type: typesEnum('type').notNull(),
	format: formatsEnum('format').notNull()
}, (configs) => [
	uniqueIndex('configs_uuid_idx').on(configs.uuid),

	index('configs_type_idx').on(configs.type),
	index('configs_format_idx').on(configs.format)
])

export const configValues = pgTable('config_values', {
	id: serial('id').primaryKey().notNull(),
	uuid: uuid('uuid').notNull().default(sql`gen_random_uuid()`),

	configId: integer('config_id').notNull().references(() => configs.id, { onDelete: 'cascade' }),

	sha1: char('sha1', { length: 40 }).notNull(),
	sha224: char('sha224', { length: 56 }).notNull(),
	sha256: char('sha256', { length: 64 }).notNull(),
	sha384: char('sha384', { length: 96 }).notNull(),
	sha512: char('sha512', { length: 128 }).notNull(),
	md5: char('md5', { length: 32 }).notNull(),

	value: text('value').notNull(),
	parsed: jsonb('parsed').default({}).notNull()
}, (configValues) => [
	index('configValues_config_idx').on(configValues.configId),
	uniqueIndex('configValues_uuid_idx').on(configValues.uuid),
	uniqueIndex('configValues_unique_config_sha512_idx').on(configValues.configId, configValues.sha512)
])

export const buildConfigs = pgTable('build_configs', {
	buildId: integer('build_id').notNull().references(() => builds.id, { onDelete: 'cascade' }),
	configId: integer('config_id').notNull().references(() => configs.id, { onDelete: 'cascade' }),
	configValueId: integer('config_value_id').notNull().references(() => configValues.id, { onDelete: 'cascade' })
}, (buildConfigs) => [
	primaryKey({ name: 'buildConfigs_pk', columns: [buildConfigs.buildId, buildConfigs.configId] }),

	index('buildConfigs_build_idx').on(buildConfigs.buildId),
	index('buildConfigs_config_idx').on(buildConfigs.configId),
	index('buildConfigs_config_value_idx').on(buildConfigs.configValueId)
])

export const chRequestStats = pgTable('ch_request_stats', {
  requestType: text('request_type').notNull(),
  searchType: text('search_type').notNull(),
  searchVersion: text('search_version').notNull(),
  buildType: text('build_type').notNull(),
  buildVersionId: text('build_version_id').notNull(),
  buildProjectVersionId: text('build_project_version_id').notNull(),
  
  totalRequests: bigint('total_requests', { mode: 'number' }).notNull(), 
  uniqueIps: bigint('unique_ips', { mode: 'number' }).notNull(),
}, (t) => [
  primaryKey({ columns: [t.requestType, t.searchType, t.searchVersion, t.buildType, t.buildVersionId, t.buildProjectVersionId] }),

  index('chRequestStats_req_search_type_idx').on(t.requestType, t.searchType),
  index('chRequestStats_req_search_ver_idx').on(t.requestType, t.searchVersion),
  index('chRequestStats_req_build_ver_idx').on(t.requestType, t.buildVersionId),
  index('chRequestStats_search_ver_idx').on(t.searchVersion), 
  index('chRequestStats_build_type_idx').on(t.buildType),
  index('chRequestStats_build_vid_idx').on(t.buildVersionId),
  index('chRequestStats_build_pvid_idx').on(t.buildProjectVersionId),
]);

export const chRequestStatsDaily = pgTable('ch_request_stats_daily', {
  requestType: text('request_type').notNull(),
  searchType: text('search_type').notNull(),
  searchVersion: text('search_version').notNull(),
  buildType: text('build_type').notNull(),
  buildVersionId: text('build_version_id').notNull(),
  buildProjectVersionId: text('build_project_version_id').notNull(),
  dateOnly: date('date_only').notNull(),
  day: smallint('day').notNull(),

  totalRequests: bigint('total_requests', { mode: 'number' }).notNull(),
  uniqueIps: bigint('unique_ips', { mode: 'number' }).notNull()
}, (t) => [
  primaryKey({ columns: [t.requestType, t.searchType, t.searchVersion, t.buildType, t.buildVersionId, t.buildProjectVersionId, t.dateOnly] }),

  index('chRequestStatsDaily_req_date_idx').on(t.requestType, t.dateOnly),
  index('chRequestStatsDaily_search_ver_date_idx').on(t.searchVersion, t.dateOnly),
  index('chRequestStatsDaily_build_ver_date_idx').on(t.buildVersionId, t.dateOnly),
  index('chRequestStatsDaily_date_idx').on(t.dateOnly),
  index('chRequestStatsDaily_day_idx').on(t.day),
]);

export const chFileStats = pgTable('ch_file_stats', {
  root: text('root').notNull(),
  path: text('path').notNull(),
  kind: text('kind').notNull(),
  extension: text('extension').notNull(),

  totalRequests: bigint('total_requests', { mode: 'number' }).notNull(),
  uniqueIps: bigint('unique_ips', { mode: 'number' }).notNull(),
  totalBytes: bigint('total_bytes', { mode: 'number' }).notNull(),
}, (t) => [
  primaryKey({ columns: [t.root, t.path, t.kind, t.extension] }),

  index('chFileStats_root_idx').on(t.root),
  index('chFileStats_kind_idx').on(t.kind),
  index('chFileStats_root_kind_idx').on(t.root, t.kind),
  index('chFileStats_extension_idx').on(t.extension),
]);

export const chFileStatsDaily = pgTable('ch_file_stats_daily', {
  root: text('root').notNull(),
  path: text('path').notNull(),
  kind: text('kind').notNull(),
  extension: text('extension').notNull(),
  dateOnly: date('date_only').notNull(),
  day: smallint('day').notNull(),

  totalRequests: bigint('total_requests', { mode: 'number' }).notNull(),
  uniqueIps: bigint('unique_ips', { mode: 'number' }).notNull(),
  totalBytes: bigint('total_bytes', { mode: 'number' }).notNull(),
}, (t) => [
  primaryKey({ columns: [t.root, t.path, t.kind, t.extension, t.dateOnly] }),

  index('chFileStatsDaily_root_date_idx').on(t.root, t.dateOnly),
  index('chFileStatsDaily_kind_date_idx').on(t.kind, t.dateOnly),
  index('chFileStatsDaily_date_idx').on(t.dateOnly),
  index('chFileStatsDaily_day_idx').on(t.day),
]);

export const nodes = pgTable('nodes', {
	name: varchar('name', { length: 255 }).primaryKey().notNull(),

	url: varchar('url', { length: 255 }).notNull(),
	version: varchar('version', { length: 255 }).notNull(),

	lastSeen: timestamp('last_seen').default(sql`now()`).notNull(),
	created: timestamp('created').default(sql`now()`).notNull()
}, (nodes) => [
	index('nodes_last_seen_idx').on(nodes.lastSeen)
])
