-- Remove deprecated shim configuration without rebuilding applications.
ALTER TABLE applications DROP COLUMN shim;
