#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub lt: ScreenPoint,
    pub rb: ScreenPoint,
}

impl ScreenRect {
    pub fn from(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        let lt_x = x1.min(x2);
        let lt_y = y1.min(y2);
        let rb_x = x1.max(x2);
        let rb_y = y1.max(y2);

        Self {
            lt: ScreenPoint { x: lt_x, y: lt_y },
            rb: ScreenPoint { x: rb_x, y: rb_y },
        }
    }

    pub fn from_points(pt1: ScreenPoint, pt2: ScreenPoint) -> Self {
        Self::from(pt1.x, pt1.y, pt2.x, pt2.y)
    }

    pub fn lt_x(&self) -> i32 {
        self.lt.x
    }

    pub fn lt_y(&self) -> i32 {
        self.lt.y
    }

    pub fn rb_x(&self) -> i32 {
        self.rb.x
    }

    pub fn rb_y(&self) -> i32 {
        self.rb.y
    }

    pub fn width(&self) -> u32 {
        (self.rb.x - self.lt.x) as u32
    }

    pub fn height(&self) -> u32 {
        (self.rb.y - self.lt.y) as u32
    }

    pub fn center(self) -> ScreenPoint {
        ScreenPoint {
            x: (self.lt_x() + self.rb_x()) / 2,
            y: (self.lt_y() + self.rb_y()) / 2,
        }
    }
}
