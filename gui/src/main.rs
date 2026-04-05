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
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tokio::time::Instant;

struct App {
    core: cosmic::Core,
    sim: Arc<RwLock<manetsim::types::SimManager<f32, 2, SimConf>>>,
    sim_canvas: SimCanvas,
    status_text: String,
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
                reset: Arc::new(Mutex::new(false)),
            },
            sim: Arc::new(RwLock::new(sim)),
            status_text: "".to_string(),
        };

        let command = app.set_window_title("Manetsim".to_string(), iced::window::Id::unique());
        (app, command)
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = canvas(self.sim_canvas.clone())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill);
        let button = widget::button::text("Tick").on_press(Message::Tick);
        let status =
            widget::row::with_children(vec![button.into(), widget::text(&self.status_text).into()]);
        widget::column::with_children(vec![canvas.into(), status.into()]).into()
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::SimData(x) => {
                self.sim_canvas.events = x.events;
                self.sim_canvas.nodes = x.nodes;
                let mut reset = self.sim_canvas.reset.lock().unwrap();
                *reset = true;

                debug!("Updated sim data");
                self.status_text = "".to_string();
                cosmic::task::none()
            }
            Message::Tick => {
                // Spawn task to tick sim
                let sim = Arc::clone(&self.sim);
                self.status_text = "running tick".to_string();
                cosmic::task::future(async move {
                    let result = tokio::task::spawn_blocking(async move || {
                        let mut sim = sim.write().await;
                        let start = Instant::now();
                        sim.n_ticks(1);
                        debug!("Tick finished after {:?}", start.elapsed());
                        SimData {
                            nodes: sim
                                .global_state_manager
                                .nodes()
                                .iter()
                                .map(|x| x.data().clone())
                                .collect(),
                            events: sim.global_state_manager.consume_stats().events(),
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
