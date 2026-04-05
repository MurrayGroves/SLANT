mod sim;
mod sim_canvas;

use crate::sim::{SimConf, generate_nodes};
use crate::sim_canvas::SimCanvas;
use cosmic::iced::application::ViewFn;
use cosmic::iced_core::id::A11yId::Widget;
use cosmic::prelude::*;
use cosmic::widget::{Canvas, canvas, container};
use cosmic::{Core, iced, widget};
use manetsim::example_behaviours::RandomWalk;
use manetsim::example_behaviours::flood::Flood;
use manetsim::propagation_models::{FreeSpace, FreeSpaceParams};
use manetsim::stats::InternalEvent;
use manetsim::types::{NodeData, SimConfig, SimManager};

struct App {
    core: cosmic::Core,
    sim: manetsim::types::SimManager<f32, 2, SimConf>,
    sim_canvas: SimCanvas,
}

#[derive(Debug, Clone)]
struct SimData {
    nodes: Vec<NodeData<f32, 2, FreeSpaceParams<f32, 2>>>,
    events: Vec<InternalEvent>,
}

#[derive(Clone, Debug)]
enum Message {
    SimData(SimData),
}

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
        let nodes = generate_nodes(1024, 3_000.0);

        let mut sim: SimManager<_, _, SimConf> = SimManager::new(nodes, 123456, FreeSpace);
        let mut app = App {
            core,
            sim_canvas: SimCanvas {
                nodes: sim
                    .global_state_manager
                    .nodes()
                    .iter()
                    .map(|x| x.data().clone())
                    .collect(),
                events: Vec::new(),
            },
            sim,
        };

        let command = app.set_window_title("Manetsim".to_string(), iced::window::Id::unique());
        (app, command)
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = canvas(self.sim_canvas.clone())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill);
        widget::column::with_children(vec![canvas.into(), widget::text("Hello world").into()])
            .into()
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::SimData(x) => {
                self.sim_canvas.events = x.events;
                self.sim_canvas.nodes = x.nodes;
            }
        }

        Task::none()
    }
}

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<App>(settings, ())
}
