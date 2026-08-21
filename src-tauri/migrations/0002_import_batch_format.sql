ALTER TABLE import_batches
ADD COLUMN file_format TEXT NOT NULL DEFAULT 'UNKNOWN'
CHECK (file_format IN ('XLSX', 'CSV', 'UNKNOWN'));

UPDATE import_batches
SET file_format = CASE
    WHEN lower(file_name) LIKE '%.xlsx' THEN 'XLSX'
    WHEN lower(file_name) LIKE '%.csv' THEN 'CSV'
    ELSE 'UNKNOWN'
END;

CREATE TRIGGER trg_import_batches_set_file_format
AFTER INSERT ON import_batches
WHEN NEW.file_format = 'UNKNOWN'
BEGIN
    UPDATE import_batches
    SET file_format = CASE
        WHEN lower(NEW.file_name) LIKE '%.xlsx' THEN 'XLSX'
        WHEN lower(NEW.file_name) LIKE '%.csv' THEN 'CSV'
        ELSE 'UNKNOWN'
    END
    WHERE id = NEW.id;
END;
