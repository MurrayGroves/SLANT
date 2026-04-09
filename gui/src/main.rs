mod sim;
mod sim_canvas;

use crate::Message::CanvasScroll;
use crate::iced::widget::button;
use crate::sim::{SimConf, generate_nodes};
use crate::sim_canvas::SimCanvas;
use cosmic::iced::alignment::Vertical;
use cosmic::iced::application::ViewFn;
use cosmic::iced::mouse::ScrollDelta;
use cosmic::iced::{Point, Vector};
use cosmic::iced_core::id::A11yId::Widget;
use cosmic::prelude::*;
use cosmic::widget::{Canvas, canvas, container};
use cosmic::{Core, iced, widget};
use log::debug;
use manetsim::example_behaviours::RandomWalk;
use manetsim::example_behaviours::flood::Flood;
use manetsim::propagation_models::{
    FreeSpace, FreeSpaceParams, SimpleDistance, SimpleDistanceParams,
};
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
    tick: usize,
    transmit_count: usize,
    link_count: usize,
}

#[derive(Debug, Clone)]
struct SimData {
    nodes: Vec<NodeData<f32, 2, SimpleDistanceParams<f32, 2>>>,
    events: Vec<InternalEvent>,
}

#[derive(Clone, Debug)]
enum Message {
    SimData(SimData),
    Tick,
    NewStatus(String),
    CanvasScroll(ScrollDelta),
    MouseDown,
    MouseUp,
    MouseMove(Point),
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
        let nodes = generate_nodes(4096, 3_000.0);

        let sim: SimManager<_, _, SimConf> = SimManager::new(nodes, 123456, SimpleDistance);
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
                reset: Arc::new(Mutex::new(0)),
                unit_ratio: Arc::new(Mutex::new(None)),
                zoom: 0.0,
                move_pos: Vector::ZERO,
                mouse_down: false,
                current_mouse_pos: None,
            },
            sim: Arc::new(RwLock::new(sim)),
            status_text: "".to_string(),
            tick: 0,
            link_count: 0,
            transmit_count: 0,
        };

        let command = app.set_window_title("Manetsim".to_string(), iced::window::Id::unique());
        (app, command)
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = widget::mouse_area(
            canvas(self.sim_canvas.clone())
                .width(iced::Length::Fill)
                .height(iced::Length::Fill),
        )
        .on_scroll(|e| CanvasScroll(e))
        .on_press(Message::MouseDown)
        .on_release(Message::MouseUp)
        .on_move(|p| Message::MouseMove(p));

        let button = widget::button::text("Tick").on_press(Message::Tick).into();
        let tick = widget::text(format!("Tick: {}", self.tick))
            .center()
            .align_y(Vertical::Center)
            .into();
        let transmit_count = widget::text(format!("Transmits: {}", self.transmit_count))
            .align_y(Vertical::Center)
            .into();
        let link_count = widget::text(format!("Receives: {}", self.link_count))
            .align_y(Vertical::Center)
            .into();
        let status = widget::row::with_children(vec![
            button,
            tick,
            transmit_count,
            link_count,
            widget::text(&self.status_text).into(),
        ])
        .align_y(Vertical::Center)
        .spacing(10);
        widget::column::with_children(vec![canvas.into(), status.into()]).into()
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::SimData(x) => {
                self.tick += 1;
                self.transmit_count = 0;
                self.link_count = 0;
                x.events.iter().for_each(|e| match e {
                    InternalEvent::PacketTransmit(_) => self.transmit_count += 1,
                    InternalEvent::PacketLink(_) => self.link_count += 1,
                });

                self.sim_canvas.events = x.events;
                self.sim_canvas.nodes = x.nodes;
                let mut reset = self.sim_canvas.reset.lock().unwrap();
                *reset = 2;

                debug!("Updated sim data");
                self.status_text = "Rendering new state".to_string();
                cosmic::task::message(Message::NewStatus("".to_string()))
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
            Message::NewStatus(x) => {
                self.status_text = x;
                cosmic::task::none()
            }
            Message::CanvasScroll(e) => {
                self.sim_canvas.zoom += match e {
                    ScrollDelta::Lines { x, y } => y,
                    ScrollDelta::Pixels { x, y } => y,
                };
                debug!("New zoom level: {}", self.sim_canvas.zoom);
                *self.sim_canvas.reset.lock().unwrap() = 1;
                cosmic::task::none()
            }
            Message::MouseDown => {
                self.sim_canvas.mouse_down = true;
                cosmic::task::none()
            }
            Message::MouseUp => {
                self.sim_canvas.mouse_down = false;
                self.sim_canvas.current_mouse_pos = None;
                cosmic::task::none()
            }
            Message::MouseMove(p) => {
                if !self.sim_canvas.mouse_down {
                    return cosmic::task::none();
                };

                if let Some(old) = self.sim_canvas.current_mouse_pos {
                    let diff = p - old;

                    self.sim_canvas.move_pos += diff;
                    *self.sim_canvas.reset.lock().unwrap() = 1;
                }
                self.sim_canvas.current_mouse_pos = Some(p);
                cosmic::task::none()
            }
        }
    }
}

fn main() -> cosmic::iced::Result {
    env_logger::init();

    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<App>(settings, ())
}
