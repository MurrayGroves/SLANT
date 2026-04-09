use crate::Message;
use cosmic::iced::mouse::Cursor;
use cosmic::iced::theme::Style;
use cosmic::iced::{Color, Point, Rectangle, Renderer, Theme};
use cosmic::iced_widget::canvas::Geometry;
use cosmic::widget::canvas;
use cosmic::widget::canvas::path::lyon_path::geom::euclid::{Transform2D, Vector2D};
use cosmic::widget::canvas::path::lyon_path::geom::{Angle, Transform, euclid};
use cosmic::widget::canvas::{Cache, Program, Stroke};
use cosmic::{iced, iced_core};
use log::debug;
use manetsim::propagation_models::FreeSpaceParams;
use manetsim::stats::InternalEvent;
use manetsim::stats::InternalEvent::PacketLink;
use manetsim::types::NodeData;
use num_traits::float::Float;
use std::cmp::min;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
struct Node {
    position: Point,
    circle: canvas::Path,
}

#[derive(Debug, Clone)]
pub struct SimCanvas {
    pub nodes: Vec<NodeData<f32, 2, FreeSpaceParams<f32, 2>>>,
    pub events: Vec<InternalEvent>,
    pub reset: Arc<Mutex<usize>>,
    /// Value to multiply sim coordinates by to get screen coordinates
    pub unit_ratio: Arc<Mutex<Option<f32>>>,
    /// Zoom step level, zero for base, higher is more zoomed in
    pub zoom: f32,
}

pub struct CanvasState {
    nodes_cache: Cache,
    transmits_caches: Mutex<Vec<Cache>>,
    /// Used to ensure that caches aren't bought into use during the 1-tick refresh used to allow state labels to update
    live_cache_count: Mutex<usize>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            nodes_cache: Cache::default(),
            transmits_caches: Mutex::new(vec![]),
            live_cache_count: Mutex::new(0),
        }
    }
}

impl CanvasState {
    fn clear(&self) {
        debug!("Clearing caches");
        self.nodes_cache.clear();
        self.transmits_caches
            .lock()
            .unwrap()
            .iter_mut()
            .for_each(|c| c.clear())
    }

    fn resize_caches(&self, num_transmits: usize) {
        let required_caches =
            (num_transmits as f32 / Self::TRANSMITS_PER_CACHE as f32).ceil() as usize;

        let mut caches = self.transmits_caches.lock().unwrap();
        if required_caches > caches.len() {
            self.nodes_cache.clear();
            debug!("Adding new cache");
            caches.iter_mut().for_each(|c| c.clear());
            caches.resize_with(required_caches, Default::default);
        }
    }

    const TRANSMITS_PER_CACHE: usize = 500_000;
}

impl SimCanvas {
    const ZOOM_EXPONENT: f32 = 1.2;
}
impl Program<Message, cosmic::Theme> for SimCanvas {
    type State = CanvasState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &cosmic::Theme,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut reset = self.reset.lock().unwrap();
        if *reset > 1 {
            *reset -= 1;
        } else if *reset == 1 {
            state.clear();
            state.resize_caches(self.events.len());
            *reset = 0;
            let mut live_caches = state.live_cache_count.lock().unwrap();
            *live_caches = (self.events.len() as f32 / CanvasState::TRANSMITS_PER_CACHE as f32)
                .ceil() as usize;
        }

        let (link_events, events): (Vec<InternalEvent>, Vec<InternalEvent>) = self
            .events
            .clone()
            .into_iter()
            .partition(|e| if let PacketLink(x) = e { true } else { false });

        let mut node_vis = Vec::new();
        let nodes = state.nodes_cache.draw(renderer, bounds.size(), |frame| {
            debug!("Drawing nodes");
            let start = Instant::now();

            let mut unit_ratio = self.unit_ratio.lock().unwrap();
            let unit_ratio = (match *unit_ratio {
                Some(x) => x,
                None => {
                    let mut min_x = f32::infinity();
                    let mut min_y = f32::infinity();
                    let mut max_x = f32::neg_infinity();
                    let mut max_y = f32::neg_infinity();

                    for node in &self.nodes {
                        if node.position[0] < min_x {
                            min_x = node.position[0];
                        }
                        if node.position[0] > max_x {
                            max_x = node.position[0];
                        }
                        if node.position[1] < min_y {
                            min_y = node.position[1];
                        }
                        if node.position[1] > max_y {
                            max_y = node.position[1];
                        }
                    }

                    let x_ratio: f32 = (bounds.size().width * 0.9 / (max_x - min_x));
                    let y_ratio: f32 = (bounds.size().height * 0.9 / (max_y - min_y));
                    let ratio = f32::min(x_ratio, y_ratio);
                    *unit_ratio = Some(ratio);
                    ratio
                }
            }) * SimCanvas::ZOOM_EXPONENT.powf(self.zoom);

            let background = canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size());
            frame.fill(&background, Color::WHITE);

            for node in &self.nodes {
                let position = Point::new(
                    node.position[0] * unit_ratio + (0.05 * bounds.size().width),
                    node.position[1] * unit_ratio + (0.05 * bounds.size().height),
                );
                let circle = canvas::Path::circle(position, 3.0);
                frame.fill(&circle, Color::BLACK);
                node_vis.push(Node { circle, position })
            }

            for event in events {
                match event {
                    InternalEvent::PacketTransmit(e) => {
                        let node = node_vis.get(e).unwrap();
                        frame.fill(&node.circle, Color::from_rgb(0.0, 1.0, 0.0));
                    }
                    InternalEvent::PacketLink(e) => {
                        panic!("Packet link in wrong vector");
                    }
                }
            }

            debug!("Took {:?} to draw canvas", start.elapsed());
        });

        let caches = state.transmits_caches.lock().unwrap();
        let live_count = state.live_cache_count.lock().unwrap();
        let transmit_geometries = if link_events.len() != 0 {
            caches
                .iter()
                .take(*live_count)
                .zip(
                    link_events
                        .chunks((link_events.len() as f32 / *live_count as f32).ceil() as usize),
                )
                .into_iter()
                .enumerate()
                .map(|(i, (cache, link_events))| {
                    cache.draw(renderer, bounds.size(), |frame| {
                        debug!(
                            "Drawing links for cache {}, with {} live caches",
                            i, *live_count
                        );
                        for e in link_events {
                            if let PacketLink(e) = e {
                                let src = node_vis.get(e.0).unwrap().position;
                                let dst = node_vis.get(e.1).unwrap().position;
                                let packet_path = canvas::Path::line(src, dst);

                                // Arrowhead
                                let diff = src - dst;
                                let length = (diff.x.powi(2) + diff.y.powi(2)).sqrt();
                                let unit = (diff / length)
                                    * 4.0
                                    * SimCanvas::ZOOM_EXPONENT.powf(self.zoom);
                                let left_wing =
                                    Transform2D::<f32, f32, f32>::rotation(Angle::degrees(-15.0))
                                        .transform_vector(Vector2D::new(unit.x, unit.y));
                                let right_wing =
                                    Transform2D::<f32, f32, f32>::rotation(Angle::degrees(15.0))
                                        .transform_vector(Vector2D::new(unit.x, unit.y));

                                frame.stroke(
                                    &packet_path,
                                    Stroke::default().with_color(Color::from_rgb(0.0, 0.0, 1.0)),
                                );

                                frame.stroke(
                                    &canvas::path::Path::line(
                                        dst,
                                        dst + iced_core::Vector::new(left_wing.x, left_wing.y),
                                    ),
                                    Stroke::default().with_color(Color::from_rgb(0.0, 0.0, 1.0)),
                                );
                                frame.stroke(
                                    &canvas::path::Path::line(
                                        dst,
                                        dst + iced_core::Vector::new(right_wing.x, right_wing.y),
                                    ),
                                    Stroke::default().with_color(Color::from_rgb(0.0, 0.0, 1.0)),
                                );
                            } else {
                                panic!("Non-packet link in wrong vector");
                            }
                        }
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut out = vec![nodes];
        out.extend(transmit_geometries);
        out
    }
}
