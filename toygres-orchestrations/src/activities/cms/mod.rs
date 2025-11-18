pub mod create_instance_record;
pub mod update_instance_state;
pub mod free_dns_name;
pub mod get_instance_by_k8s_name;
pub mod get_instance_connection;
pub mod record_health_check;
pub mod update_instance_health;
pub mod record_instance_actor;
pub mod delete_instance_record;

mod db;

pub(crate) use db::get_pool;

