use std::collections::VecDeque;

pub const LOG_CAP: usize = 200;
const BUILD_COST: i32 = 10;
const HARVEST_ENERGY: f32 = 10.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WorldRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub struct Game {
    pub gold: i32,
    pub drones: i32,
    pub energy: f32,
    pub auto_harvest: bool,
    auto_clock: f32,
    pub log: VecDeque<String>,
    pub log_seq: usize,
    pub selected: Option<usize>,
    ship: [f32; 2],
    target: [f32; 2],
    time: f32,
}

impl Game {
    pub fn new(width: f32, height: f32) -> Self {
        let mut game = Self {
            gold: 0,
            drones: 0,
            energy: 100.0,
            auto_harvest: false,
            auto_clock: 0.0,
            log: VecDeque::new(),
            log_seq: 0,
            selected: None,
            ship: [width * 0.5, height * 0.6],
            target: [width * 0.5, height * 0.6],
            time: 0.0,
        };
        game.log("Ship online");
        game
    }

    fn log(&mut self, text: impl Into<String>) {
        self.log.push_front(text.into());
        self.log.truncate(LOG_CAP);
        self.log_seq += 1;
        self.selected = self.selected.map(|row| (row + 1).min(LOG_CAP - 1));
    }

    pub fn can_harvest(&self) -> bool {
        self.energy >= HARVEST_ENERGY
    }

    pub fn can_build(&self) -> bool {
        self.gold >= BUILD_COST
    }

    pub fn harvest(&mut self) {
        if !self.can_harvest() {
            return;
        }
        self.energy -= HARVEST_ENERGY;
        self.gold += 1;
        self.log(format!("Harvested +1 (gold {})", self.gold));
    }

    pub fn build(&mut self) {
        if !self.can_build() {
            return;
        }
        self.gold -= BUILD_COST;
        self.drones += 1;
        self.log(format!("Built drone #{}", self.drones));
    }

    pub fn toggle_auto(&mut self) {
        self.auto_harvest = !self.auto_harvest;
        self.log(if self.auto_harvest {
            "Auto-harvest on"
        } else {
            "Auto-harvest off"
        });
    }

    pub fn select(&mut self, row: usize) {
        if row < self.log.len() {
            self.selected = (self.selected != Some(row)).then_some(row);
        }
    }

    pub fn world_click(&mut self, x: f32, y: f32) {
        self.target = [x, y];
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.energy = (self.energy + 15.0 * dt).min(100.0);
        let k = 1.0 - (-6.0 * dt).exp();
        for i in 0..2 {
            self.ship[i] += (self.target[i] - self.ship[i]) * k;
        }
        if self.auto_harvest && self.drones > 0 {
            self.auto_clock += dt;
            if self.auto_clock >= 1.0 {
                self.auto_clock -= 1.0;
                self.gold += self.drones;
            }
        }
    }

    pub fn rects(&self) -> Vec<WorldRect> {
        let mut out = Vec::with_capacity(2 + self.drones as usize);
        out.push(WorldRect {
            x: self.target[0] - 3.0,
            y: self.target[1] - 3.0,
            w: 6.0,
            h: 6.0,
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.35,
        });
        out.push(WorldRect {
            x: self.ship[0] - 14.0,
            y: self.ship[1] - 14.0,
            w: 28.0,
            h: 28.0,
            r: 0.95,
            g: 0.55,
            b: 0.2,
            a: 1.0,
        });
        for i in 0..self.drones {
            let angle = self.time * 1.7 + i as f32 * 2.39996;
            out.push(WorldRect {
                x: self.ship[0] + angle.cos() * 44.0 - 4.0,
                y: self.ship[1] + angle.sin() * 44.0 - 4.0,
                w: 8.0,
                h: 8.0,
                r: 0.3,
                g: 0.8,
                b: 0.95,
                a: 1.0,
            });
        }
        out
    }
}
