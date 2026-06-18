-- organizations
CREATE TABLE organizations (
  id          CHAR(36)     NOT NULL PRIMARY KEY,
  name        VARCHAR(160) NOT NULL,
  slug        VARCHAR(160) NOT NULL UNIQUE,
  created_at  DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at  DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- users  (email is globally unique; login resolves the org)
CREATE TABLE users (
  id              CHAR(36)     NOT NULL PRIMARY KEY,
  organization_id CHAR(36)     NOT NULL,
  email           VARCHAR(255) NOT NULL UNIQUE,
  password_hash   VARCHAR(255) NOT NULL,
  full_name       VARCHAR(160) NOT NULL,
  role            VARCHAR(16)  NOT NULL,              -- owner|admin|engineer|viewer
  is_active       BOOLEAN      NOT NULL DEFAULT TRUE,
  created_at      DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at      DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_users_org (organization_id),
  CONSTRAINT fk_users_org FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- refresh tokens (rotation + revocation; store SHA-256 hash, never the raw token)
CREATE TABLE refresh_tokens (
  id         CHAR(36)    NOT NULL PRIMARY KEY,
  user_id    CHAR(36)    NOT NULL,
  token_hash CHAR(64)    NOT NULL UNIQUE,             -- sha256 hex
  expires_at DATETIME(6) NOT NULL,
  revoked_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  KEY idx_rt_user (user_id),
  CONSTRAINT fk_rt_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- plans (pricing catalog)
CREATE TABLE plans (
  id                     CHAR(36)    NOT NULL PRIMARY KEY,
  code                   VARCHAR(16) NOT NULL UNIQUE,  -- basic|live|predict
  name                   VARCHAR(80) NOT NULL,
  price_per_machine_year BIGINT      NOT NULL,         -- paise
  onboarding_fee         BIGINT      NOT NULL DEFAULT 0,-- paise, one-time per machine
  currency               CHAR(3)     NOT NULL DEFAULT 'INR',
  features               JSON        NULL,
  is_active              BOOLEAN     NOT NULL DEFAULT TRUE,
  created_at             DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- subscription (one per org; tracks status + billing period)
CREATE TABLE subscriptions (
  id                   CHAR(36)    NOT NULL PRIMARY KEY,
  organization_id      CHAR(36)    NOT NULL UNIQUE,
  status               VARCHAR(16) NOT NULL,           -- trialing|active|past_due|canceled
  trial_ends_at        DATETIME(6) NULL,
  current_period_start DATETIME(6) NULL,
  current_period_end   DATETIME(6) NULL,
  created_at           DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at           DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  CONSTRAINT fk_sub_org FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- machines  (core identity is mutable; rich specs live in versioned documents)
CREATE TABLE machines (
  id                       CHAR(36)     NOT NULL PRIMARY KEY,
  organization_id          CHAR(36)     NOT NULL,
  plan_id                  CHAR(36)     NULL,          -- this machine's pricing tier
  name                     VARCHAR(160) NOT NULL,
  make                     VARCHAR(120) NULL,
  model                    VARCHAR(120) NULL,
  serial_number            VARCHAR(120) NULL,
  asset_tag                VARCHAR(64)  NULL,
  location                 VARCHAR(160) NULL,
  year_installed           SMALLINT     NULL,
  status                   VARCHAR(16)  NOT NULL DEFAULT 'active', -- active|maintenance|retired
  primary_photo_version_id CHAR(36)     NULL,          -- convenience pointer
  created_by               CHAR(36)     NULL,
  created_at               DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at               DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_machines_org (organization_id),
  CONSTRAINT fk_machines_org  FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE,
  CONSTRAINT fk_machines_plan FOREIGN KEY (plan_id)
    REFERENCES plans(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- documents : a logical, versioned artifact slot on a machine (THE generic part)
CREATE TABLE documents (
  id                 CHAR(36)     NOT NULL PRIMARY KEY,
  machine_id         CHAR(36)     NOT NULL,
  category           VARCHAR(40)  NOT NULL,   -- see DocumentCategory enum
  name               VARCHAR(200) NOT NULL,   -- human label (required when category=other)
  storage_kind       VARCHAR(8)   NOT NULL,   -- file | json
  current_version_no INT          NOT NULL DEFAULT 0,
  created_by         CHAR(36)     NULL,
  created_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_documents_machine (machine_id),
  KEY idx_documents_machine_cat (machine_id, category),
  CONSTRAINT fk_documents_machine FOREIGN KEY (machine_id)
    REFERENCES machines(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- document_versions : immutable snapshots. INSERT only -- never UPDATE the payload.
CREATE TABLE document_versions (
  id                CHAR(36)     NOT NULL PRIMARY KEY,
  document_id       CHAR(36)     NOT NULL,
  version_no        INT          NOT NULL,   -- 1,2,3...
  is_current        BOOLEAN      NOT NULL DEFAULT FALSE,
  -- file payload (when documents.storage_kind = 'file')
  storage_key       VARCHAR(512) NULL,
  original_filename VARCHAR(255) NULL,
  mime_type         VARCHAR(160) NULL,
  size_bytes        BIGINT       NULL,
  checksum_sha256   CHAR(64)     NULL,
  -- json payload (when documents.storage_kind = 'json'): specs, parameters, BOM rows
  content_json      JSON         NULL,
  -- common
  change_note       VARCHAR(500) NULL,        -- "what changed" / changelog line
  metadata          JSON         NULL,        -- typed extras: {o_number, controller, plc_model,...}
  created_by        CHAR(36)     NULL,
  created_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  UNIQUE KEY uq_docver (document_id, version_no),
  KEY idx_docver_current (document_id, is_current),
  CONSTRAINT fk_docver_document FOREIGN KEY (document_id)
    REFERENCES documents(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
