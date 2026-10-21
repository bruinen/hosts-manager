mod host_manager;
mod app;
mod db_manager;
mod profile_view;
mod dns_lookup;
mod privileges;

use app::init_app;

pub fn main() -> iced::Result {
    init_app()
}
