PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sources (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  root_path TEXT NOT NULL UNIQUE,
  source_type TEXT NOT NULL DEFAULT 'local_fs',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS photos (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  storage_file_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  file_name TEXT NOT NULL,
  file_ext TEXT,
  file_size INTEGER NOT NULL,
  mime_type TEXT,
  content_hash TEXT,
  shot_at TEXT,
  created_at_fs TEXT,
  modified_at_fs TEXT,
  sort_time TEXT NOT NULL,
  width INTEGER,
  height INTEGER,
  exif_json TEXT,
  gps_lat REAL,
  gps_lng REAL,
  remark TEXT,
  deleted_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (source_id) REFERENCES sources (id),
  UNIQUE (source_id, storage_file_id),
  UNIQUE (source_id, file_path)
);

CREATE TABLE IF NOT EXISTS albums (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  remark TEXT,
  cover_photo_id TEXT,
  auto_created INTEGER NOT NULL DEFAULT 0 CHECK (auto_created IN (0, 1)),
  album_date TEXT,
  rule_key TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (cover_photo_id) REFERENCES photos (id)
);

CREATE TABLE IF NOT EXISTS photo_albums (
  photo_id TEXT NOT NULL,
  album_id TEXT NOT NULL,
  seq_no INTEGER,
  created_at TEXT NOT NULL,
  PRIMARY KEY (photo_id, album_id),
  FOREIGN KEY (photo_id) REFERENCES photos (id),
  FOREIGN KEY (album_id) REFERENCES albums (id)
);

CREATE TABLE IF NOT EXISTS scan_jobs (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'failed')),
  started_at TEXT,
  finished_at TEXT,
  total_count INTEGER,
  new_count INTEGER,
  updated_count INTEGER,
  failed_count INTEGER,
  error_message TEXT,
  FOREIGN KEY (source_id) REFERENCES sources (id)
);

CREATE TABLE IF NOT EXISTS photo_features (
  photo_id TEXT PRIMARY KEY,
  version INTEGER NOT NULL,
  features_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (photo_id) REFERENCES photos (id)
);

CREATE INDEX IF NOT EXISTS idx_photos_sort_time ON photos (sort_time DESC);
CREATE INDEX IF NOT EXISTS idx_photos_shot_at ON photos (shot_at);
CREATE INDEX IF NOT EXISTS idx_photos_file_name ON photos (file_name);
CREATE INDEX IF NOT EXISTS idx_photos_source_path ON photos (source_id, file_path);
CREATE INDEX IF NOT EXISTS idx_photo_albums_album ON photo_albums (album_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_albums_created_at ON albums (created_at DESC);
