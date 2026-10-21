mod host_manager;
mod app;
mod db_manager;
mod style;
mod view;
mod dns_lookup;
mod dns_cache;
mod privileges;

use app::init_app;

pub fn main() -> iced::Result {
    init_app()
}
