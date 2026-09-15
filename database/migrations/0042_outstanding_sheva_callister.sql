CREATE TABLE "nodes" (
	"name" varchar(255) PRIMARY KEY NOT NULL,
	"url" varchar(255) NOT NULL,
	"version" varchar(255) NOT NULL,
	"last_seen" timestamp DEFAULT now() NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE INDEX "nodes_last_seen_idx" ON "nodes" USING btree ("last_seen");