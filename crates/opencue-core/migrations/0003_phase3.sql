-- Phase 3: files shared in chats. JSON array of {id, name, mime, size}; the bytes live in files/<id>/.
ALTER TABLE messages ADD COLUMN attachments TEXT;
