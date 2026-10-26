-- Groups come in kinds: a plain chat, an AI council, or a coding group tied to a repository.
ALTER TABLE groups ADD COLUMN kind TEXT NOT NULL DEFAULT 'chat';
ALTER TABLE groups ADD COLUMN config TEXT NOT NULL DEFAULT '{}';
