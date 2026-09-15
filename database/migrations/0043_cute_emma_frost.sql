DO $$
DECLARE
	canonical text;
	existing text;
BEGIN
	FOREACH canonical IN ARRAY ARRAY[
		'buildHashes_build_idx',
		'buildHashes_primary_idx',
		'buildHashes_sha1_idx',
		'buildHashes_sha224_idx',
		'buildHashes_sha256_idx',
		'buildHashes_sha384_idx',
		'buildHashes_sha512_idx',
		'buildHashes_md5_idx'
	] LOOP
		SELECT indexname INTO existing
		FROM pg_indexes
		WHERE schemaname = 'public'
			AND tablename = 'build_hashes'
			AND lower(indexname) = lower(canonical);

		IF existing IS NULL THEN
			RAISE EXCEPTION 'index % not found on build_hashes', canonical;
		END IF;

		IF existing <> canonical THEN
			EXECUTE format('ALTER INDEX public.%I RENAME TO %I', existing, canonical);
		END IF;
	END LOOP;
END $$;--> statement-breakpoint
ALTER INDEX "buildHashes_sha1_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER INDEX "buildHashes_sha224_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER INDEX "buildHashes_sha256_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER INDEX "buildHashes_sha384_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER INDEX "buildHashes_sha512_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER INDEX "buildHashes_md5_idx" SET (fillfactor=90);--> statement-breakpoint
ALTER TABLE "builds" SET (autovacuum_vacuum_insert_scale_factor = 0.02);--> statement-breakpoint
ALTER TABLE "build_hashes" SET (autovacuum_vacuum_insert_scale_factor = 0.02);
