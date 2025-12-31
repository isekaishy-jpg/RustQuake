use qw_common::collision::{BoxHull, Trace, hull_point_contents, trace_hull};
use qw_common::{
    BspCollision, CONTENTS_EMPTY, CONTENTS_SOLID, CONTENTS_WATER, MoveVars, UserCmd, Vec3,
};

const MAX_PHYSENTS: usize = 32;
const MAX_CLIP_PLANES: usize = 5;
const STEPSIZE: f32 = 18.0;
const STOP_EPSILON: f32 = 0.1;
const BUTTON_JUMP: u8 = 2;

const PLAYER_MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);
const PLAYER_MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);

#[derive(Debug, Clone)]
pub struct PhysEnt {
    pub origin: Vec3,
    pub model: Option<usize>,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub info: i32,
}

#[derive(Debug, Clone)]
pub struct PlayerMove {
    pub origin: Vec3,
    pub angles: Vec3,
    pub velocity: Vec3,
    pub oldbuttons: i32,
    pub waterjumptime: f32,
    pub dead: bool,
    pub spectator: bool,
    pub cmd: UserCmd,
    pub physents: Vec<PhysEnt>,
    pub onground: i32,
    pub waterlevel: i32,
    pub watertype: i32,
    pub touch: Vec<i32>,
}

impl PlayerMove {
    pub fn new(cmd: UserCmd) -> Self {
        Self {
            origin: Vec3::default(),
            angles: cmd.angles,
            velocity: Vec3::default(),
            oldbuttons: 0,
            waterjumptime: 0.0,
            dead: false,
            spectator: false,
            cmd,
            physents: Vec::new(),
            onground: -1,
            waterlevel: 0,
            watertype: CONTENTS_EMPTY,
            touch: Vec::new(),
        }
    }

    pub fn add_world(&mut self, model_index: usize) {
        if self.physents.is_empty() {
            self.physents.push(PhysEnt {
                origin: Vec3::default(),
                model: Some(model_index),
                mins: Vec3::default(),
                maxs: Vec3::default(),
                info: 0,
            });
        } else {
            self.physents[0] = PhysEnt {
                origin: Vec3::default(),
                model: Some(model_index),
                mins: Vec3::default(),
                maxs: Vec3::default(),
                info: 0,
            };
        }
    }

    pub fn add_physent(&mut self, ent: PhysEnt) {
        if self.physents.len() < MAX_PHYSENTS {
            self.physents.push(ent);
        }
    }

    pub fn simulate(&mut self, collision: &BspCollision, movevars: MoveVars) {
        let mut ctx = PmoveContext::new(self, collision, movevars);
        ctx.player_move();
    }
}

#[derive(Debug, Copy, Clone)]
struct MoveTrace {
    trace: Trace,
    ent: i32,
}

struct PmoveContext<'a> {
    pmove: &'a mut PlayerMove,
    collision: &'a BspCollision,
    movevars: MoveVars,
    frametime: f32,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
}

impl<'a> PmoveContext<'a> {
    fn new(pmove: &'a mut PlayerMove, collision: &'a BspCollision, movevars: MoveVars) -> Self {
        Self {
            pmove,
            collision,
            movevars,
            frametime: 0.0,
            forward: Vec3::default(),
            right: Vec3::default(),
            up: Vec3::default(),
        }
    }

    fn player_move(&mut self) {
        self.frametime = self.pmove.cmd.msec as f32 * 0.001;
        self.pmove.touch.clear();
        self.pmove.angles = self.pmove.cmd.angles;

        let (forward, right, up) = angle_vectors(self.pmove.angles);
        self.forward = forward;
        self.right = right;
        self.up = up;

        if self.pmove.spectator {
            self.spectator_move();
            return;
        }

        self.nudge_position();
        self.pmove.angles = self.pmove.cmd.angles;
        self.categorize_position();

        if self.pmove.waterlevel == 2 {
            self.check_water_jump();
        }

        if self.pmove.velocity.z < 0.0 {
            self.pmove.waterjumptime = 0.0;
        }

        if (self.pmove.cmd.buttons & BUTTON_JUMP) != 0 {
            self.jump_button();
        } else {
            self.pmove.oldbuttons &= !(BUTTON_JUMP as i32);
        }

        self.apply_friction();

        if self.pmove.waterlevel >= 2 {
            self.water_move();
        } else {
            self.air_move();
        }

        self.categorize_position();
    }

    fn pm_clip_velocity(in_vec: Vec3, normal: Vec3, overbounce: f32) -> (Vec3, i32) {
        let mut blocked = 0;
        if normal.z > 0.0 {
            blocked |= 1;
        }
        if normal.z == 0.0 {
            blocked |= 2;
        }

        let backoff = in_vec.dot(normal) * overbounce;
        let mut out = Vec3::new(
            in_vec.x - normal.x * backoff,
            in_vec.y - normal.y * backoff,
            in_vec.z - normal.z * backoff,
        );
        if out.x > -STOP_EPSILON && out.x < STOP_EPSILON {
            out.x = 0.0;
        }
        if out.y > -STOP_EPSILON && out.y < STOP_EPSILON {
            out.y = 0.0;
        }
        if out.z > -STOP_EPSILON && out.z < STOP_EPSILON {
            out.z = 0.0;
        }
        (out, blocked)
    }

    fn pm_fly_move(&mut self) -> i32 {
        let numbumps = 4;
        let mut blocked = 0;
        let original_velocity = self.pmove.velocity;
        let primal_velocity = self.pmove.velocity;
        let mut numplanes = 0;
        let mut planes = [Vec3::default(); MAX_CLIP_PLANES];
        let mut time_left = self.frametime;

        for _ in 0..numbumps {
            let end = vec_add(self.pmove.origin, vec_scale(self.pmove.velocity, time_left));
            let trace = self.pm_player_move(self.pmove.origin, end);

            if trace.trace.startsolid || trace.trace.allsolid {
                self.pmove.velocity = Vec3::default();
                return 3;
            }

            if trace.trace.fraction > 0.0 {
                self.pmove.origin = trace.trace.endpos;
                numplanes = 0;
            }

            if trace.trace.fraction == 1.0 {
                break;
            }

            if self.pmove.touch.len() < MAX_PHYSENTS {
                self.pmove.touch.push(trace.ent);
            }

            if trace.trace.plane.normal.z > 0.7 {
                blocked |= 1;
            }
            if trace.trace.plane.normal.z == 0.0 {
                blocked |= 2;
            }

            time_left -= time_left * trace.trace.fraction;
            if numplanes >= MAX_CLIP_PLANES {
                self.pmove.velocity = Vec3::default();
                break;
            }

            planes[numplanes] = trace.trace.plane.normal;
            numplanes += 1;

            let mut new_velocity = self.pmove.velocity;
            let mut i = 0;
            while i < numplanes {
                let (clipped, _) = Self::pm_clip_velocity(original_velocity, planes[i], 1.0);
                new_velocity = clipped;
                let mut j = 0;
                while j < numplanes {
                    if j != i && new_velocity.dot(planes[j]) < 0.0 {
                        break;
                    }
                    j += 1;
                }
                if j == numplanes {
                    break;
                }
                i += 1;
            }

            if i != numplanes {
                self.pmove.velocity = new_velocity;
            } else if numplanes == 2 {
                let dir = vec_cross(planes[0], planes[1]);
                let d = dir.dot(self.pmove.velocity);
                self.pmove.velocity = vec_scale(dir, d);
            } else {
                self.pmove.velocity = Vec3::default();
                break;
            }

            if self.pmove.velocity.dot(primal_velocity) <= 0.0 {
                self.pmove.velocity = Vec3::default();
                break;
            }
        }

        if self.pmove.waterjumptime > 0.0 {
            self.pmove.velocity = primal_velocity;
        }

        blocked
    }

    fn pm_ground_move(&mut self) {
        self.pmove.velocity.z = 0.0;
        if vec_is_zero(self.pmove.velocity) {
            return;
        }

        let dest = Vec3::new(
            self.pmove.origin.x + self.pmove.velocity.x * self.frametime,
            self.pmove.origin.y + self.pmove.velocity.y * self.frametime,
            self.pmove.origin.z,
        );

        let trace = self.pm_player_move(self.pmove.origin, dest);
        if trace.trace.fraction == 1.0 {
            self.pmove.origin = trace.trace.endpos;
            return;
        }

        let original = self.pmove.origin;
        let originalvel = self.pmove.velocity;

        self.pm_fly_move();

        let down = self.pmove.origin;
        let downvel = self.pmove.velocity;

        self.pmove.origin = original;
        self.pmove.velocity = originalvel;

        let mut dest = self.pmove.origin;
        dest.z += STEPSIZE;
        let trace = self.pm_player_move(self.pmove.origin, dest);
        if !trace.trace.startsolid && !trace.trace.allsolid {
            self.pmove.origin = trace.trace.endpos;
        }

        self.pm_fly_move();

        let mut dest = self.pmove.origin;
        dest.z -= STEPSIZE;
        let trace = self.pm_player_move(self.pmove.origin, dest);
        if trace.trace.plane.normal.z < 0.7 {
            self.pmove.origin = down;
            self.pmove.velocity = downvel;
            return;
        }
        if !trace.trace.startsolid && !trace.trace.allsolid {
            self.pmove.origin = trace.trace.endpos;
        }
        let up = self.pmove.origin;

        let downdist = (down.x - original.x) * (down.x - original.x)
            + (down.y - original.y) * (down.y - original.y);
        let updist =
            (up.x - original.x) * (up.x - original.x) + (up.y - original.y) * (up.y - original.y);

        if downdist > updist {
            self.pmove.origin = down;
            self.pmove.velocity = downvel;
        } else {
            self.pmove.velocity.z = downvel.z;
        }
    }

    fn apply_friction(&mut self) {
        if self.pmove.waterjumptime > 0.0 {
            return;
        }

        let speed = vec_length(self.pmove.velocity);
        if speed < 1.0 {
            self.pmove.velocity.x = 0.0;
            self.pmove.velocity.y = 0.0;
            return;
        }

        let mut friction = self.movevars.friction;
        if self.pmove.onground != -1 {
            let mut start = vec_add(
                self.pmove.origin,
                vec_scale(self.pmove.velocity, 16.0 / speed),
            );
            let mut stop = start;
            start.z = self.pmove.origin.z + PLAYER_MINS.z;
            stop.z = start.z - 34.0;

            let trace = self.pm_player_move(start, stop);
            if trace.trace.fraction == 1.0 {
                friction *= 2.0;
            }
        }

        let mut drop = 0.0;
        if self.pmove.waterlevel >= 2 {
            drop +=
                speed * self.movevars.waterfriction * self.pmove.waterlevel as f32 * self.frametime;
        } else if self.pmove.onground != -1 {
            let control = if speed < self.movevars.stopspeed {
                self.movevars.stopspeed
            } else {
                speed
            };
            drop += control * friction * self.frametime;
        }

        let mut newspeed = speed - drop;
        if newspeed < 0.0 {
            newspeed = 0.0;
        }
        newspeed /= speed;

        self.pmove.velocity = vec_scale(self.pmove.velocity, newspeed);
    }

    fn pm_accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
        if self.pmove.dead || self.pmove.waterjumptime > 0.0 {
            return;
        }
        let currentspeed = self.pmove.velocity.dot(wishdir);
        let addspeed = wishspeed - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = accel * self.frametime * wishspeed;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        self.pmove.velocity = vec_add(self.pmove.velocity, vec_scale(wishdir, accelspeed));
    }

    fn pm_air_accelerate(&mut self, wishdir: Vec3, wishspeed: f32, accel: f32) {
        if self.pmove.dead || self.pmove.waterjumptime > 0.0 {
            return;
        }
        let wishspd = wishspeed.min(30.0);
        let currentspeed = self.pmove.velocity.dot(wishdir);
        let addspeed = wishspd - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = accel * wishspeed * self.frametime;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        self.pmove.velocity = vec_add(self.pmove.velocity, vec_scale(wishdir, accelspeed));
    }

    fn water_move(&mut self) {
        let mut wishvel = Vec3::default();
        wishvel.x = self.forward.x * self.pmove.cmd.forwardmove as f32
            + self.right.x * self.pmove.cmd.sidemove as f32;
        wishvel.y = self.forward.y * self.pmove.cmd.forwardmove as f32
            + self.right.y * self.pmove.cmd.sidemove as f32;
        wishvel.z = self.forward.z * self.pmove.cmd.forwardmove as f32
            + self.right.z * self.pmove.cmd.sidemove as f32;

        if self.pmove.cmd.forwardmove == 0
            && self.pmove.cmd.sidemove == 0
            && self.pmove.cmd.upmove == 0
        {
            wishvel.z -= 60.0;
        } else {
            wishvel.z += self.pmove.cmd.upmove as f32;
        }

        let (wishdir, mut wishspeed) = vec_normalize(wishvel);
        if wishspeed > self.movevars.maxspeed {
            wishspeed = self.movevars.maxspeed;
        }
        wishspeed *= 0.7;

        self.pm_accelerate(wishdir, wishspeed, self.movevars.wateraccelerate);

        let dest = vec_add(
            self.pmove.origin,
            vec_scale(self.pmove.velocity, self.frametime),
        );
        let mut start = dest;
        start.z += STEPSIZE + 1.0;
        let trace = self.pm_player_move(start, dest);
        if !trace.trace.startsolid && !trace.trace.allsolid {
            self.pmove.origin = trace.trace.endpos;
            return;
        }

        self.pm_fly_move();
    }

    fn air_move(&mut self) {
        let mut wishvel = Vec3::default();
        let fmove = self.pmove.cmd.forwardmove as f32;
        let smove = self.pmove.cmd.sidemove as f32;

        let mut forward = self.forward;
        let mut right = self.right;
        forward.z = 0.0;
        right.z = 0.0;
        let (forward, _) = vec_normalize(forward);
        let (right, _) = vec_normalize(right);

        wishvel.x = forward.x * fmove + right.x * smove;
        wishvel.y = forward.y * fmove + right.y * smove;

        let (wishdir, mut wishspeed) = vec_normalize(wishvel);
        if wishspeed > self.movevars.maxspeed {
            wishspeed = self.movevars.maxspeed;
        }

        if self.pmove.onground != -1 {
            self.pmove.velocity.z = 0.0;
            self.pm_accelerate(wishdir, wishspeed, self.movevars.accelerate);
            self.pmove.velocity.z -=
                self.movevars.entgravity * self.movevars.gravity * self.frametime;
            self.pm_ground_move();
        } else {
            self.pm_air_accelerate(wishdir, wishspeed, self.movevars.accelerate);
            self.pmove.velocity.z -=
                self.movevars.entgravity * self.movevars.gravity * self.frametime;
            self.pm_fly_move();
        }
    }

    fn categorize_position(&mut self) {
        let mut point = self.pmove.origin;
        point.z -= 1.0;
        if self.pmove.velocity.z > 180.0 {
            self.pmove.onground = -1;
        } else {
            let trace = self.pm_player_move(self.pmove.origin, point);
            if trace.trace.plane.normal.z < 0.7 {
                self.pmove.onground = -1;
            } else {
                self.pmove.onground = trace.ent;
            }
            if self.pmove.onground != -1 {
                self.pmove.waterjumptime = 0.0;
                if !trace.trace.startsolid && !trace.trace.allsolid {
                    self.pmove.origin = trace.trace.endpos;
                }
            }

            if trace.ent > 0 && self.pmove.touch.len() < MAX_PHYSENTS {
                self.pmove.touch.push(trace.ent);
            }
        }

        self.pmove.waterlevel = 0;
        self.pmove.watertype = CONTENTS_EMPTY;

        point = Vec3::new(
            self.pmove.origin.x,
            self.pmove.origin.y,
            self.pmove.origin.z + PLAYER_MINS.z + 1.0,
        );
        let mut cont = self.pm_point_contents(point);
        if cont <= CONTENTS_WATER {
            self.pmove.watertype = cont;
            self.pmove.waterlevel = 1;
            point.z = self.pmove.origin.z + (PLAYER_MINS.z + PLAYER_MAXS.z) * 0.5;
            cont = self.pm_point_contents(point);
            if cont <= CONTENTS_WATER {
                self.pmove.waterlevel = 2;
                point.z = self.pmove.origin.z + 22.0;
                cont = self.pm_point_contents(point);
                if cont <= CONTENTS_WATER {
                    self.pmove.waterlevel = 3;
                }
            }
        }
    }

    fn jump_button(&mut self) {
        if self.pmove.dead {
            self.pmove.oldbuttons |= BUTTON_JUMP as i32;
            return;
        }

        if self.pmove.waterjumptime > 0.0 {
            self.pmove.waterjumptime -= self.frametime;
            if self.pmove.waterjumptime < 0.0 {
                self.pmove.waterjumptime = 0.0;
            }
            return;
        }

        if self.pmove.waterlevel >= 2 {
            self.pmove.onground = -1;
            self.pmove.velocity.z = match self.pmove.watertype {
                CONTENTS_WATER => 100.0,
                qw_common::CONTENTS_SLIME => 80.0,
                _ => 50.0,
            };
            return;
        }

        if self.pmove.onground == -1 {
            return;
        }
        if (self.pmove.oldbuttons & BUTTON_JUMP as i32) != 0 {
            return;
        }

        self.pmove.onground = -1;
        self.pmove.velocity.z += 270.0;
        self.pmove.oldbuttons |= BUTTON_JUMP as i32;
    }

    fn check_water_jump(&mut self) {
        if self.pmove.waterjumptime > 0.0 {
            return;
        }
        if self.pmove.velocity.z < -180.0 {
            return;
        }

        let mut flatforward = self.forward;
        flatforward.z = 0.0;
        let (flatforward, _) = vec_normalize(flatforward);

        let mut spot = vec_add(self.pmove.origin, vec_scale(flatforward, 24.0));
        spot.z += 8.0;
        let cont = self.pm_point_contents(spot);
        if cont != CONTENTS_SOLID {
            return;
        }
        spot.z += 24.0;
        let cont = self.pm_point_contents(spot);
        if cont != CONTENTS_EMPTY {
            return;
        }

        self.pmove.velocity = vec_scale(flatforward, 50.0);
        self.pmove.velocity.z = 310.0;
        self.pmove.waterjumptime = 2.0;
        self.pmove.oldbuttons |= BUTTON_JUMP as i32;
    }

    fn nudge_position(&mut self) {
        let base = self.pmove.origin;
        let mut origin = self.pmove.origin;
        origin.x = (origin.x * 8.0) as i32 as f32 * 0.125;
        origin.y = (origin.y * 8.0) as i32 as f32 * 0.125;
        origin.z = (origin.z * 8.0) as i32 as f32 * 0.125;
        self.pmove.origin = origin;

        if self.test_player_position(self.pmove.origin) {
            return;
        }

        let sign = [0.0, -1.0 / 8.0, 1.0 / 8.0];
        for &dz in &sign {
            for &dx in &sign {
                for &dy in &sign {
                    let candidate = Vec3::new(base.x + dx, base.y + dy, base.z + dz);
                    if self.test_player_position(candidate) {
                        self.pmove.origin = candidate;
                        return;
                    }
                }
            }
        }

        self.pmove.origin = base;
    }

    fn spectator_move(&mut self) {
        let speed = vec_length(self.pmove.velocity);
        if speed < 1.0 {
            self.pmove.velocity = Vec3::default();
        } else {
            let friction = self.movevars.friction * 1.5;
            let control = if speed < self.movevars.stopspeed {
                self.movevars.stopspeed
            } else {
                speed
            };
            let drop = control * friction * self.frametime;
            let mut newspeed = speed - drop;
            if newspeed < 0.0 {
                newspeed = 0.0;
            }
            let scale = newspeed / speed;
            self.pmove.velocity = vec_scale(self.pmove.velocity, scale);
        }

        let fmove = self.pmove.cmd.forwardmove as f32;
        let smove = self.pmove.cmd.sidemove as f32;
        let mut wishvel = Vec3::default();
        let (forward, _) = vec_normalize(self.forward);
        let (right, _) = vec_normalize(self.right);
        wishvel.x = forward.x * fmove + right.x * smove;
        wishvel.y = forward.y * fmove + right.y * smove;
        wishvel.z = forward.z * fmove + right.z * smove + self.pmove.cmd.upmove as f32;
        let (wishdir, mut wishspeed) = vec_normalize(wishvel);
        if wishspeed > self.movevars.spectatormaxspeed {
            wishspeed = self.movevars.spectatormaxspeed;
        }
        let currentspeed = self.pmove.velocity.dot(wishdir);
        let addspeed = wishspeed - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = self.movevars.accelerate * self.frametime * wishspeed;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        self.pmove.velocity = vec_add(self.pmove.velocity, vec_scale(wishdir, accelspeed));
        self.pmove.origin = vec_add(
            self.pmove.origin,
            vec_scale(self.pmove.velocity, self.frametime),
        );
    }

    fn pm_point_contents(&self, point: Vec3) -> i32 {
        let Some(hull) = self.collision.hull(0, 0) else {
            return CONTENTS_EMPTY;
        };
        hull_point_contents(&hull, hull.firstclipnode, point)
    }

    fn test_player_position(&self, pos: Vec3) -> bool {
        for ent in &self.pmove.physents {
            if let Some(model_index) = ent.model {
                let Some(hull) = self.collision.hull(model_index, 1) else {
                    continue;
                };
                let test = vec_sub(pos, ent.origin);
                if hull_point_contents(&hull, hull.firstclipnode, test) == CONTENTS_SOLID {
                    return false;
                }
            } else {
                let mins = vec_sub(ent.mins, PLAYER_MAXS);
                let maxs = vec_sub(ent.maxs, PLAYER_MINS);
                let box_hull = BoxHull::new(mins, maxs);
                let hull = box_hull.hull();
                let test = vec_sub(pos, ent.origin);
                if hull_point_contents(&hull, hull.firstclipnode, test) == CONTENTS_SOLID {
                    return false;
                }
            }
        }
        true
    }

    fn pm_player_move(&self, start: Vec3, end: Vec3) -> MoveTrace {
        let mut total = Trace {
            fraction: 1.0,
            endpos: end,
            ..Trace::default()
        };
        let mut total_ent = -1;

        for (index, ent) in self.pmove.physents.iter().enumerate() {
            let offset = ent.origin;
            let start_l = vec_sub(start, offset);
            let end_l = vec_sub(end, offset);

            let (trace, ent_index) = if let Some(model_index) = ent.model {
                let Some(hull) = self.collision.hull(model_index, 1) else {
                    continue;
                };
                (trace_hull(&hull, start_l, end_l), index as i32)
            } else {
                let mins = vec_sub(ent.mins, PLAYER_MAXS);
                let maxs = vec_sub(ent.maxs, PLAYER_MINS);
                let box_hull = BoxHull::new(mins, maxs);
                let hull = box_hull.hull();
                (trace_hull(&hull, start_l, end_l), index as i32)
            };

            if trace.fraction < total.fraction {
                let endpos = vec_add(trace.endpos, offset);
                total = Trace { endpos, ..trace };
                total_ent = ent_index;
            }
        }

        MoveTrace {
            trace: total,
            ent: total_ent,
        }
    }
}

fn angle_vectors(angles: Vec3) -> (Vec3, Vec3, Vec3) {
    let (pitch, yaw, roll) = (
        angles.x.to_radians(),
        angles.y.to_radians(),
        angles.z.to_radians(),
    );
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();

    let forward = Vec3::new(cp * cy, cp * sy, -sp);
    let right = Vec3::new(-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp);
    let up = Vec3::new(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp);
    (forward, right, up)
}

fn vec_add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn vec_sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn vec_scale(a: Vec3, scale: f32) -> Vec3 {
    Vec3::new(a.x * scale, a.y * scale, a.z * scale)
}

fn vec_length(a: Vec3) -> f32 {
    (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
}

fn vec_is_zero(a: Vec3) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

fn vec_normalize(a: Vec3) -> (Vec3, f32) {
    let len = vec_length(a);
    if len == 0.0 {
        (Vec3::default(), 0.0)
    } else {
        (vec_scale(a, 1.0 / len), len)
    }
}

fn vec_cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_vectors_forward_on_yaw() {
        let (forward, right, up) = angle_vectors(Vec3::new(0.0, 90.0, 0.0));
        assert!((forward.x - 0.0).abs() < 0.001);
        assert!((forward.y - 1.0).abs() < 0.001);
        assert!((forward.z - 0.0).abs() < 0.001);
        assert!((right.x - 1.0).abs() < 0.001);
        assert!((right.y - 0.0).abs() < 0.001);
        assert!((up.z - 1.0).abs() < 0.001);
    }

    #[test]
    fn clip_velocity_blocks_floor() {
        let in_vec = Vec3::new(0.0, 0.0, -10.0);
        let normal = Vec3::new(0.0, 0.0, 1.0);
        let (out, blocked) = PmoveContext::pm_clip_velocity(in_vec, normal, 1.0);
        assert_eq!(blocked, 1);
        assert_eq!(out.z, 0.0);
    }
}
