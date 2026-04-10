use crate::Message;
use crate::sim::SeqPacketTransmit;
use cosmic::iced::mouse::Cursor;
use cosmic::iced::theme::Style;
use cosmic::iced::{Color, Point, Rectangle, Renderer, Theme, Vector};
use cosmic::iced_renderer::geometry::frame::Backend;
use cosmic::iced_widget::canvas::Geometry;
use cosmic::widget::canvas;
use cosmic::widget::canvas::path::lyon_path::geom::euclid::{Transform2D, Vector2D};
use cosmic::widget::canvas::path::lyon_path::geom::{Angle, Transform, euclid};
use cosmic::widget::canvas::{Cache, Frame, Program, Stroke};
use cosmic::{iced, iced_core};
use log::debug;
use manetsim::propagation_models::{FreeSpaceParams, SimpleDistanceParams};
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
    pub nodes: Vec<NodeData<f32, 2, SimpleDistanceParams<f32, 2>>>,
    pub events: Vec<InternalEvent>,
    pub seq_transmits: Vec<SeqPacketTransmit<u16>>,
    pub reset: Arc<Mutex<usize>>,
    /// Value to multiply sim coordinates by to get screen coordinates
    pub unit_ratio: Arc<Mutex<Option<f32>>>,
    /// Zoom step level, zero for base, higher is more zoomed in
    pub zoom: f32,
    pub move_pos: Vector,
    pub mouse_down: bool,
    pub current_mouse_pos: Option<Point>,
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

    const TRANSMITS_PER_CACHE: usize = 300_000;
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
        let mut frame = Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size());
        frame.fill(&background, Color::WHITE);

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

            for node in &self.nodes {
                let position = Point::new(
                    node.position[0] * unit_ratio + (0.05 * bounds.size().width),
                    node.position[1] * unit_ratio + (0.05 * bounds.size().height),
                ) + self.move_pos;

                let circle = canvas::Path::circle(position, 3.0);
                if bounds.contains(position) {
                    frame.fill(&circle, Color::BLACK);
                }
                node_vis.push(Node { circle, position })
            }

            for event in &self.seq_transmits {
                let node = node_vis.get(event.node).unwrap();
                if bounds.contains(node.position) {
                    // Convert seq to HSL to RGB
                    let norm: f32 = event.seq as f32 / u16::MAX as f32;

                    let h = norm * 6.0;
                    let sector = h as usize;

                    let x = 1.0 - ((h % 2.0) - 1.0).abs();

                    let (r, g, b) = match sector {
                        0 => (1.0, x, 0.0),
                        1 => (x, 1.0, 0.0),
                        2 => (0.0, 1.0, x),
                        3 => (0.0, x, 1.0),
                        4 => (x, 0.0, 1.0),
                        _ => (1.0, 0.0, x), // Sector 5 (and edge case 6.0)
                    };

                    let r = (r * 255.0) as u8;
                    let g = (g * 255.0) as u8;
                    let b = (b * 255.0) as u8;
                    frame.fill(&node.circle, Color::from_rgb8(r, g, b));
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
                                if !bounds.contains(src) && !bounds.contains(dst) {
                                    continue;
                                }

                                let packet_path = canvas::Path::line(src, dst);

                                // Arrowhead
                                let diff = src - dst;
                                let length = (diff.x.powi(2) + diff.y.powi(2)).sqrt();
                                let unit = (diff / length)
                                    * 8.0
                                    * SimCanvas::ZOOM_EXPONENT.powf(self.zoom);
                                let left_wing =
                                    Transform2D::<f32, f32, f32>::rotation(Angle::degrees(-8.0))
                                        .transform_vector(Vector2D::new(unit.x, unit.y));
                                let right_wing =
                                    Transform2D::<f32, f32, f32>::rotation(Angle::degrees(8.0))
                                        .transform_vector(Vector2D::new(unit.x, unit.y));

                                // let mut colour = Color::from_rgb(
                                //     rand.random::<f32>(),
                                //     rand.random::<f32>(),
                                //     rand.random::<f32>(),
                                // );
                                //
                                // while !colour.is_readable_on(Color::WHITE) {
                                //     colour = Color::from_rgb(
                                //         rand.random::<f32>(),
                                //         rand.random::<f32>(),
                                //         rand.random::<f32>(),
                                //     );
                                // }
                                let colour = Color::from_rgba(0.0, 0.0, 1.0, 0.2);

                                frame.stroke(&packet_path, Stroke::default().with_color(colour));

                                let left_wing = dst + Vector::new(left_wing.x, left_wing.y);
                                let right_wing = dst + Vector::new(right_wing.x, right_wing.y);

                                let tri = canvas::path::Path::new(|path| {
                                    path.move_to(left_wing);
                                    path.line_to(right_wing);
                                    path.line_to(dst)
                                });
                                frame.fill(&tri, colour);
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

        let mut out = vec![frame.into_geometry()];
        out.extend(transmit_geometries);
        out.push(nodes);
        out
    }
}
