mod host_manager;
mod app;
mod db_manager;
mod profile_view;


use app::init_app;

pub fn main() -> iced::Result {
    init_app()
}
