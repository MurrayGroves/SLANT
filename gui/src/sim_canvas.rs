use crate::Message;
use cosmic::iced;
use cosmic::iced::mouse::Cursor;
use cosmic::iced::theme::Style;
use cosmic::iced::{Color, Point, Rectangle, Renderer, Theme};
use cosmic::iced_widget::canvas::Geometry;
use cosmic::widget::canvas;
use cosmic::widget::canvas::{Cache, Program, Stroke};
use log::debug;
use manetsim::propagation_models::FreeSpaceParams;
use manetsim::stats::InternalEvent;
use manetsim::types::NodeData;
use num_traits::float::Float;
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
}

impl Program<Message, cosmic::Theme> for SimCanvas {
    type State = Cache;

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
            *reset = 0;
        }

        let geometry = state.draw(renderer, bounds.size(), |frame| {
            let start = Instant::now();

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

            let mut node_vis = Vec::new();

            let x_ratio: f32 = (bounds.size().width * 0.9 / (max_x - min_x));
            let y_ratio: f32 = (bounds.size().height * 0.9 / (max_y - min_y));
            let background = canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size());
            frame.fill(&background, Color::WHITE);

            for node in &self.nodes {
                let position = Point::new(
                    (node.position[0] - min_x) * x_ratio + (0.05 * bounds.size().width),
                    (node.position[1] - min_y) * y_ratio + (0.05 * bounds.size().height),
                );
                let circle = canvas::Path::circle(position, 1.0);
                frame.fill(&circle, Color::BLACK);
                node_vis.push(Node { circle, position })
            }

            for event in &self.events {
                match event {
                    InternalEvent::PacketTransmit(e) => {
                        let node = node_vis.get(*e).unwrap();
                        frame.fill(&node.circle, Color::from_rgb(0.0, 1.0, 0.0));
                    }
                    InternalEvent::PacketLink(e) => {
                        let src = node_vis.get(e.0).unwrap().position;
                        let dst = node_vis.get(e.1).unwrap().position;
                        let packet_path = canvas::Path::line(src, dst);
                        frame.stroke(
                            &packet_path,
                            Stroke::default().with_color(Color::from_rgb(0.0, 1.0, 0.0)),
                        );
                    }
                }
            }

            debug!("Took {:?} to draw canvas", start.elapsed());
        });

        vec![geometry]
    }
}
