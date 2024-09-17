-- Add migration scrpt here
alter table staff add column deleted timestamptz null;

alter table staff drop constraint unique_staff_name_customer;
alter table staff add constraint unique_staff_name_customer unique nulls not distinct (name, customer_id, deleted);
