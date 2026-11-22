-- A reading is one metric and one number, and metric ids are snake_case. No deployment held
-- data worth keeping, so readings, limits and sensors start over instead of being renamed.
truncate readings, thresholds, sensors;
truncate readings_hourly;
alter table readings drop column payload;
