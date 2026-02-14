//! garcalc-geometry: Dynamic geometry system
//!
//! Provides geometric primitives, constructions, constraints,
//! and measurements for interactive geometry.
//!
//! This is a stub for Sprint 5 implementation.

use serde::{Deserialize, Serialize};

/// A 2D point
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

/// A unique identifier for shapes
pub type ShapeId = u64;

/// Geometric shapes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Shape {
    Point(Point2D),
    Line {
        p1: Point2D,
        p2: Point2D,
    },
    Segment {
        p1: Point2D,
        p2: Point2D,
    },
    Ray {
        origin: Point2D,
        direction: Point2D,
    },
    Circle {
        center: Point2D,
        radius: f64,
    },
    Arc {
        center: Point2D,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Polygon(Vec<Point2D>),
}

/// Geometric constraints for dynamic geometry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    PointOnLine(ShapeId, ShapeId),
    PointOnCircle(ShapeId, ShapeId),
    Perpendicular(ShapeId, ShapeId),
    Parallel(ShapeId, ShapeId),
    Tangent(ShapeId, ShapeId),
    Coincident(ShapeId, ShapeId),
    Fixed(ShapeId),
}

/// Geometry canvas state
#[derive(Default)]
pub struct GeometryCanvas {
    pub shapes: Vec<(ShapeId, Shape)>,
    pub constraints: Vec<Constraint>,
    next_id: ShapeId,
}

impl GeometryCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_shape(&mut self, shape: Shape) -> ShapeId {
        let id = self.next_id;
        self.next_id += 1;
        self.shapes.push((id, shape));
        id
    }
}
