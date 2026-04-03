use cosmic::prelude::*;
use cosmic::{Core, iced, widget};

struct App {
    core: cosmic::Core,
}

#[derive(Clone, Debug)]
enum Message {}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &str = "dev.murrax.manetsim";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        let mut app = App { core };

        let command = app.set_window_title("Manetsim".to_string(), iced::window::Id::unique());
        (app, command)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        widget::text("Manetsim").into()
    }
}

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<App>(settings, ())
}
