pub mod create_instance_record;
pub mod update_instance_state;
pub mod free_dns_name;
pub mod get_instance_by_k8s_name;

mod db;

pub(crate) use db::get_pool;

