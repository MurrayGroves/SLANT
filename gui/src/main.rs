mod sim;
mod sim_canvas;

use crate::iced::widget::button;
use crate::sim::{SimConf, generate_nodes};
use crate::sim_canvas::SimCanvas;
use cosmic::iced::application::ViewFn;
use cosmic::iced_core::id::A11yId::Widget;
use cosmic::prelude::*;
use cosmic::widget::{Canvas, canvas, container};
use cosmic::{Core, iced, widget};
use log::debug;
use manetsim::example_behaviours::RandomWalk;
use manetsim::example_behaviours::flood::Flood;
use manetsim::propagation_models::{FreeSpace, FreeSpaceParams};
use manetsim::stats::InternalEvent;
use manetsim::types::{NodeData, SimConfig, SimManager};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;

struct App {
    core: cosmic::Core,
    sim: Arc<RwLock<manetsim::types::SimManager<f32, 2, SimConf>>>,
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
    Tick,
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

        let sim: SimManager<_, _, SimConf> = SimManager::new(nodes, 123456, FreeSpace);
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
            sim: Arc::new(RwLock::new(sim)),
        };

        let command = app.set_window_title("Manetsim".to_string(), iced::window::Id::unique());
        (app, command)
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = canvas(self.sim_canvas.clone())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill);
        let button = widget::button::text("Tick").on_press(Message::Tick);
        widget::column::with_children(vec![canvas.into(), button.into()]).into()
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::SimData(x) => {
                self.sim_canvas.events = x.events;
                self.sim_canvas.nodes = x.nodes;
                debug!("Updated sim data");
                println!("updated sim data");
                cosmic::task::none()
            }
            Message::Tick => {
                // Spawn task to tick sim
                let sim = Arc::clone(&self.sim);
                cosmic::task::future(async move {
                    let result = tokio::task::spawn_blocking(async move || {
                        let mut sim = sim.write().await;
                        sim.n_ticks(1);
                        SimData {
                            nodes: sim
                                .global_state_manager
                                .nodes()
                                .iter()
                                .map(|x| x.data().clone())
                                .collect(),
                            events: vec![],
                        }
                    })
                    .await
                    .expect("Failed to tick sim")
                    .await;

                    Message::SimData(result)
                })
            }
        }
    }
}

fn main() -> cosmic::iced::Result {
    env_logger::init();

    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<App>(settings, ())
}
