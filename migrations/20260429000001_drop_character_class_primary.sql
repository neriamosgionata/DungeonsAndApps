-- Character classes are now modeled inside sheet.classes[] (name, subclass,
-- level), supporting multiclass + level breakdown. The legacy scalar
-- class_primary column is no longer read by the app.
alter table characters drop column if exists class_primary;
