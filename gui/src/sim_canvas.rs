use crate::Message;
use cosmic::iced::mouse::Cursor;
use cosmic::iced::{Color, Point, Rectangle, Renderer, Theme};
use cosmic::iced_widget::canvas::Geometry;
use cosmic::widget::canvas;
use cosmic::widget::canvas::Program;
use manetsim::propagation_models::FreeSpaceParams;
use manetsim::stats::InternalEvent;
use manetsim::types::NodeData;
use num_traits::float::Float;

#[derive(Debug, Clone)]
pub struct SimCanvas {
    pub nodes: Vec<NodeData<f32, 2, FreeSpaceParams<f32, 2>>>,
    pub events: Vec<InternalEvent>,
}

impl Program<Message, cosmic::Theme> for SimCanvas {
    type State = ();

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &cosmic::Theme,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

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
        let background = canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size());
        frame.fill(&background, Color::WHITE);
        for node in &self.nodes {
            let circle = canvas::Path::circle(
                Point::new(
                    (node.position[0] - min_x) * x_ratio + (0.05 * bounds.size().width),
                    (node.position[1] - min_y) * y_ratio + (0.05 * bounds.size().height),
                ),
                1.0,
            );
            frame.fill(&circle, Color::BLACK)
        }
        vec![frame.into_geometry()]
    }
}
